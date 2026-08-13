import 'dart:io';

import 'package:flutter/foundation.dart';

import '../api/api_client.dart';
import '../models.dart';
import '../offline/offline_models.dart';
import '../offline/offline_store.dart';
import 'session_store.dart';

enum SessionStatus {
  booting,
  unauthenticated,

  /// Connecte, mais avec un mot de passe temporaire : rien d'autre n'est accessible
  /// tant qu'il n'a pas ete remplace.
  mustChangePassword,
  authenticated,
}

class SessionController extends ChangeNotifier {
  SessionController({
    required this.api,
    required this.store,
    OfflineCacheStore? cache,
  }) : _cache = cache ?? SecureOfflineCacheStore();

  final ApmtrackApi api;
  final SessionStore store;
  final OfflineCacheStore _cache;

  static const _cacheKey = 'offline.snapshot';
  static const _draftsKey = 'offline.drafts';
  static const _maxCachedPvs = 50;

  SessionStatus status = SessionStatus.booting;
  AuthSession? session;
  MobileProfile? profile;
  List<Intervention> interventions = const [];
  List<Pv> pvs = const [];
  PatrouilleActive activePatrouille = const PatrouilleActive(agents: []);
  List<Patrouille> patrouilles = const [];

  /// Full persisted draft queue, all users of the device included.
  List<PvDraft> _allDrafts = const [];
  String? message;
  bool loadingData = false;

  /// True when the last sync failed and the UI is showing cached data.
  bool offline = false;
  bool syncing = false;
  int _draftCounter = 0;

  String? get token => session?.accessToken;
  bool get isAuthenticated => status == SessionStatus.authenticated;

  /// Drafts of the signed-in user only. The queue survives logout on a shared
  /// device, so another user's drafts must neither be shown nor synced (the
  /// server derives the agent from the session token) — they stay persisted
  /// until their author signs back in. Legacy drafts without owner tag are
  /// attributed to the current user.
  List<PvDraft> get drafts =>
      _allDrafts.where(_ownsDraft).toList(growable: false);

  /// Replaces the whole persisted queue (all owners) — test seeding seam.
  @visibleForTesting
  set drafts(List<PvDraft> value) => _allDrafts = value;

  bool get hasPendingDrafts =>
      drafts.any((draft) => draft.status == PvDraftStatus.pending);

  bool _ownsDraft(PvDraft draft) =>
      draft.ownerUserId == null || draft.ownerUserId == session?.user.id;

  Future<void> bootstrap() async {
    status = SessionStatus.booting;
    notifyListeners();
    final saved = await store.read();
    if (saved == null) {
      status = SessionStatus.unauthenticated;
      notifyListeners();
      return;
    }
    session = saved;
    // Show cached data immediately so the app is usable before/without network.
    await _loadFromCache();
    // refreshData never throws: a transient network error keeps the session
    // (online-first read cache), only a definitive auth rejection clears it.
    await refreshData();
    status = _resolveStatus();
    notifyListeners();
  }

  Future<void> login(String email, String password) async {
    message = null;
    notifyListeners();
    final nextSession = await api.login(email, password);
    session = nextSession;
    await store.write(nextSession);
    // Reload the persisted queue so the returning author's pending drafts
    // resync in this session (signOut cleared the in-memory list).
    _allDrafts = decodeDrafts(await _cache.read(_draftsKey));
    await refreshData();
    status = _resolveStatus();
    notifyListeners();
  }

  SessionStatus _resolveStatus() {
    final current = session;
    if (current == null) {
      return SessionStatus.unauthenticated;
    }
    return current.user.mustChangePassword
        ? SessionStatus.mustChangePassword
        : SessionStatus.authenticated;
  }

  /// Remplace le mot de passe temporaire d'un compte provisionne.
  ///
  /// Le serveur revoque tous les refresh tokens : la session courante ne peut plus etre
  /// prolongee, l'agent est donc renvoye vers l'ecran de connexion pour repartir sur des
  /// jetons coherents avec son nouveau mot de passe.
  Future<void> changePassword(String currentPassword, String newPassword) async {
    final accessToken = token;
    if (accessToken == null) {
      return;
    }
    await api.changePassword(accessToken, currentPassword, newPassword);
    await signOut(localOnly: true);
    message = 'Mot de passe enregistre. Reconnectez-vous.';
    notifyListeners();
  }

  Future<void> refreshData() async {
    if (token == null) {
      return;
    }
    loadingData = true;
    notifyListeners();

    Object? lastError;
    Future<void> load(Future<void> Function() body) async {
      try {
        await body();
      } catch (error) {
        lastError = error;
      }
    }

    await Future.wait([
      load(() async => profile = await _withAuth(api.mobileMe)),
      load(
        () async => interventions = await _withAuth(api.mobileInterventions),
      ),
      load(
        () async => activePatrouille = await _withAuth(api.activePatrouille),
      ),
      load(() async => patrouilles = await _withAuth(api.mobilePatrouilles)),
      load(() async => pvs = (await _withAuth((t) => api.pvs(t))).items),
    ]);

    loadingData = false;
    if (session == null) {
      // Refresh failed for good: signOut() has already cleared session.
      offline = false;
      message = _messageFor(lastError) ?? 'Session expiree';
      notifyListeners();
      return;
    }

    offline = lastError != null;
    message = _messageFor(lastError);
    if (lastError == null) {
      // Fresh server data: persist it for offline reads and flush the queue.
      await _saveSnapshot();
      notifyListeners();
      await syncDrafts();
    } else {
      notifyListeners();
    }
  }

  /// Creates a PV. Online it goes straight to the server; offline (network
  /// failure) it is queued as a local draft. A business rejection (4xx) is
  /// surfaced as an error and never queued.
  Future<CreatePvOutcome> createPv(
    CreatePvPayload payload, {
    List<PvDraftPhoto> photos = const [],
  }) async {
    try {
      final pv = await _withAuth((token) => api.createPv(token, payload));
      try {
        await _uploadDraftPhotos(pv.id, photos);
      } catch (error) {
        if (_isNetworkError(error)) {
          final draft = await _enqueueDraft(
            payload,
            photos: photos,
            serverPvId: pv.id,
          );
          return CreatePvOutcome.queued(draft);
        }
        rethrow;
      }
      await refreshData();
      return CreatePvOutcome.synced(pv);
    } catch (error) {
      if (_isNetworkError(error)) {
        final draft = await _enqueueDraft(payload, photos: photos);
        return CreatePvOutcome.queued(draft);
      }
      rethrow;
    }
  }

  Future<Pv> updatePv(String pvId, CreatePvPayload payload) async {
    final pv = await _withAuth((token) => api.updatePv(token, pvId, payload));
    await refreshData();
    return pv;
  }

  Future<void> recordPatrouillePosition({
    required String patrouilleId,
    required double latitude,
    required double longitude,
    double? accuracyM,
  }) async {
    await _withAuth(
      (token) => api.recordPatrouillePosition(
        token,
        patrouilleId,
        latitude: latitude,
        longitude: longitude,
        accuracyM: accuracyM,
      ),
    );
    await refreshData();
  }

  /// Sends a single patrouille position without triggering a full data refresh.
  /// Used by the automatic tracker, which fires frequently while on patrol.
  Future<void> pushPatrouillePosition({
    required String patrouilleId,
    required double latitude,
    required double longitude,
    double? accuracyM,
  }) {
    return _withAuth(
      (token) => api.recordPatrouillePosition(
        token,
        patrouilleId,
        latitude: latitude,
        longitude: longitude,
        accuracyM: accuracyM,
      ),
    );
  }

  Future<String> pvQrSvg(String pvId) =>
      _withAuth((token) => api.pvQrSvg(token, pvId));

  Future<List<int>> pvPdfBytes(String pvId) =>
      _withAuth((token) => api.pvPdfBytes(token, pvId));

  Future<List<PvPhoto>> pvPhotos(String pvId) =>
      _withAuth((token) => api.listPvPhotos(token, pvId));

  Future<PvPhoto> uploadPvPhoto(
    String pvId, {
    required List<int> bytes,
    required String filename,
    required String contentType,
  }) {
    return _withAuth(
      (token) => api.uploadPvPhoto(
        token,
        pvId,
        bytes: bytes,
        filename: filename,
        contentType: contentType,
      ),
    );
  }

  Future<void> deletePvPhoto(String pvId, String photoId) =>
      _withAuth((token) => api.deletePvPhoto(token, pvId, photoId));

  /// URL + headers for loading a photo through the authenticated API
  /// (used by `Image.network`).
  String photoContentUrl(String pvId, String photoId) =>
      api.photoContentUrl(pvId, photoId);

  String agentPhotoContentUrl(String agentId) =>
      api.agentPhotoContentUrl(agentId);

  Map<String, String> get authHeaders {
    final currentToken = token;
    return currentToken == null
        ? const {}
        : {'authorization': 'Bearer $currentToken'};
  }

  // ───────────────────────────── Offline drafts ──────────────────────────────

  /// Pushes pending PV drafts to the server, oldest first. Stops on a network
  /// error (retried later); marks a draft failed on a definitive rejection.
  Future<void> syncDrafts() async {
    if (syncing || token == null || !hasPendingDrafts) {
      return;
    }
    syncing = true;
    notifyListeners();

    // Iterate over a frozen snapshot of the current user's drafts; mutate a
    // separate working list (full queue, other users' drafts included) so we
    // never modify the collection being iterated. Draft instances are shared,
    // so status changes are reflected in `remaining`.
    final pending = List<PvDraft>.from(drafts);
    final remaining = List<PvDraft>.from(_allDrafts);
    var draftsChanged = false;
    var pvsChanged = false;
    for (final draft in pending) {
      if (draft.status != PvDraftStatus.pending) {
        continue;
      }
      try {
        var serverPvId = draft.serverPvId;
        Pv? pv;
        if (serverPvId == null) {
          final createdPv = await _withAuth(
            (token) => api.createPv(token, draft.payload),
          );
          pv = createdPv;
          serverPvId = createdPv.id;
          draft.serverPvId = serverPvId;
          draftsChanged = true;
          await _saveDrafts();
        }
        await _uploadDraftPhotos(serverPvId, draft.photos);
        remaining.removeWhere((item) => item.localId == draft.localId);
        if (pv != null) {
          pvs = [pv, ...pvs];
        }
        draftsChanged = true;
        pvsChanged = pv != null || pvsChanged;
      } catch (error) {
        if (_isNetworkError(error)) {
          // Still offline — keep the queue and retry on the next sync.
          break;
        }
        // Definitive rejection (agent suspendu, référentiel modifié,
        // double-verbalisation, montant invalide…): mark and keep for review.
        draft.status = PvDraftStatus.failed;
        draft.error = _messageFor(error) ?? 'Rejet serveur';
        draftsChanged = true;
      }
    }

    _allDrafts = remaining;
    syncing = false;
    if (draftsChanged) {
      await _saveDrafts();
    }
    if (pvsChanged) {
      await _saveSnapshot();
    }
    notifyListeners();
  }

  /// Re-queues a failed draft for the next sync attempt.
  Future<void> retryDraft(String localId) async {
    _allDrafts = _allDrafts.map((draft) {
      if (draft.localId == localId) {
        draft.status = PvDraftStatus.pending;
        draft.error = null;
      }
      return draft;
    }).toList();
    await _saveDrafts();
    notifyListeners();
    await syncDrafts();
  }

  Future<void> deleteDraft(String localId) async {
    _allDrafts = _allDrafts
        .where((draft) => draft.localId != localId)
        .toList();
    await _saveDrafts();
    notifyListeners();
  }

  Future<PvDraft> _enqueueDraft(
    CreatePvPayload payload, {
    List<PvDraftPhoto> photos = const [],
    String? serverPvId,
  }) async {
    Intervention? intervention;
    for (final item in interventions) {
      if (item.id == payload.interventionId) {
        intervention = item;
        break;
      }
    }
    final draft = PvDraft(
      localId: _newLocalId(),
      payload: payload,
      createdAt: DateTime.now(),
      ownerUserId: session?.user.id,
      photos: photos,
      serverPvId: serverPvId,
      interventionName: intervention?.nom,
      amountFcfa: intervention?.montantFcfa,
    );
    _allDrafts = [..._allDrafts, draft];
    offline = true;
    await _saveDrafts();
    notifyListeners();
    return draft;
  }

  Future<void> _loadFromCache() async {
    final snapshot = OfflineSnapshot.decode(await _cache.read(_cacheKey));
    if (snapshot != null) {
      if (snapshot.profile != null) {
        profile = snapshot.profile;
      }
      if (snapshot.interventions.isNotEmpty) {
        interventions = snapshot.interventions;
      }
      pvs = snapshot.pvs;
      if (snapshot.patrouille != null) {
        activePatrouille = snapshot.patrouille!;
      }
      patrouilles = snapshot.patrouilles;
    }
    _allDrafts = decodeDrafts(await _cache.read(_draftsKey));
    notifyListeners();
  }

  Future<void> _saveSnapshot() async {
    final snapshot = OfflineSnapshot(
      profile: profile,
      interventions: interventions,
      pvs: pvs.take(_maxCachedPvs).toList(),
      patrouille: activePatrouille,
      patrouilles: patrouilles,
    );
    await _cache.write(_cacheKey, snapshot.encode());
  }

  Future<void> _saveDrafts() async {
    await _cache.write(_draftsKey, encodeDrafts(_allDrafts));
  }

  bool _isNetworkError(Object error) {
    if (error is ApiException) {
      // API-mapped errors carry an HTTP status; a null status means the request
      // never reached the server (timeout / no route to host).
      return error.statusCode == null;
    }
    // Lower-level client/socket exceptions are always connectivity failures.
    return true;
  }

  String? _messageFor(Object? error) {
    if (error == null) {
      return null;
    }
    if (error is ApiException) {
      return error.message;
    }
    return 'Reseau indisponible';
  }

  String _newLocalId() =>
      'draft-${DateTime.now().microsecondsSinceEpoch}-${_draftCounter++}';

  Future<void> _uploadDraftPhotos(
    String pvId,
    List<PvDraftPhoto> photos,
  ) async {
    for (final photo in photos) {
      final file = File(photo.path);
      if (!await file.exists()) {
        continue;
      }
      final bytes = await file.readAsBytes();
      await uploadPvPhoto(
        pvId,
        bytes: bytes,
        filename: photo.filename,
        contentType: photo.contentType,
      );
      try {
        await file.delete();
      } catch (_) {
        // A failed cleanup must not block field work synchronization.
      }
    }
  }

  /// Runs an authenticated call. On a 401, attempts a single token refresh
  /// (shared across concurrent calls) and replays the request once.
  Future<T> _withAuth<T>(Future<T> Function(String token) call) async {
    final currentToken = token;
    if (currentToken == null) {
      throw ApiException('Session expiree', statusCode: 401);
    }
    try {
      return await call(currentToken);
    } on ApiException catch (error) {
      if (!error.isUnauthorized) {
        rethrow;
      }
      await _refreshSession();
      final renewedToken = token;
      if (renewedToken == null) {
        throw ApiException('Session expiree', statusCode: 401);
      }
      return await call(renewedToken);
    }
  }

  Future<void>? _refreshing;

  Future<void> _refreshSession() {
    return _refreshing ??= _performRefresh().whenComplete(
      () => _refreshing = null,
    );
  }

  Future<void> _performRefresh() async {
    final current = session;
    if (current == null) {
      throw ApiException('Session expiree', statusCode: 401);
    }
    try {
      final next = await api.refresh(current.refreshToken);
      session = next;
      // The backend rotates refresh tokens, so the new pair must be persisted
      // or the next refresh would fail against a revoked token.
      await store.write(next);
      notifyListeners();
    } on ApiException catch (error) {
      // Only a definitive rejection invalidates the session; a transient
      // network failure is rethrown without destroying the local session.
      if (error.isUnauthorized) {
        await signOut(localOnly: true);
        message = 'Session expiree';
        notifyListeners();
      }
      rethrow;
    }
  }

  Future<PvPublic> verifyPv(String pvNumber) => api.verifyPublicPv(pvNumber);

  Future<void> signOut({bool localOnly = false}) async {
    final current = session;
    session = null;
    profile = null;
    interventions = const [];
    pvs = const [];
    activePatrouille = const PatrouilleActive(agents: []);
    patrouilles = const [];
    _allDrafts = const [];
    offline = false;
    status = SessionStatus.unauthenticated;
    await store.clear();
    // Drop the PII read cache, but keep the draft queue persisted so unsynced
    // field work survives a session expiry / logout and resyncs at next login.
    await _cache.delete(_cacheKey);
    if (!localOnly && current != null) {
      try {
        await api.logout(current.accessToken, current.refreshToken);
      } catch (_) {
        // Local logout must remain possible even if the API is down.
      }
    }
    notifyListeners();
  }
}
