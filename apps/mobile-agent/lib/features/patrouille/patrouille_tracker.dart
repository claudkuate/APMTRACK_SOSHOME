import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../core/api/api_client.dart';
import '../../core/auth/session_controller.dart';
import 'location_source.dart';

/// Automatic patrol tracking (foreground while-in-use).
///
/// While active it streams GPS fixes and pushes each one to the server. Fixes
/// that fail to send (offline / transient error) are queued and replayed on the
/// next successful flush, so a connectivity drop never loses the trace.
///
/// Note: this tracks while the app is in the foreground. True background
/// tracking (app closed / screen off) requires a native foreground service and
/// is a separate increment.
class PatrouilleTracker extends ChangeNotifier {
  PatrouilleTracker({
    required this.controller,
    LocationSource? locationSource,
    this.distanceFilterM = 25,
    this.maxQueue = 200,
  }) : _locationSource = locationSource ?? const GeolocatorLocationSource();

  final SessionController controller;
  final LocationSource _locationSource;
  final int distanceFilterM;
  final int maxQueue;

  String? _patrouilleId;
  StreamSubscription<GpsFix>? _subscription;
  final List<GpsFix> _queue = [];
  bool _flushing = false;

  bool get isTracking => _subscription != null;
  int get pendingCount => _queue.length;
  DateTime? lastSentAt;
  String? error;

  /// Starts tracking for [patrouilleId]. No-op if already tracking.
  Future<void> start(String patrouilleId) async {
    if (isTracking) {
      return;
    }
    error = null;
    notifyListeners();
    try {
      await _locationSource.ensureReady();
    } on ApiException catch (failure) {
      error = failure.message;
      notifyListeners();
      return;
    }
    _patrouilleId = patrouilleId;
    _subscription = _locationSource
        .watch(distanceFilterM: distanceFilterM)
        .listen(
          _onFix,
          onError: (Object failure) {
            error = failure is ApiException
                ? failure.message
                : 'Suivi GPS interrompu';
            notifyListeners();
          },
        );
    notifyListeners();
  }

  /// Stops tracking and clears the pending queue.
  Future<void> stop() async {
    await _subscription?.cancel();
    _subscription = null;
    _patrouilleId = null;
    _queue.clear();
    notifyListeners();
  }

  Future<void> _onFix(GpsFix fix) async {
    _queue.add(fix);
    if (_queue.length > maxQueue) {
      // Keep the most recent fixes; drop the oldest to bound memory.
      _queue.removeRange(0, _queue.length - maxQueue);
    }
    notifyListeners();
    await _flush();
  }

  Future<void> _flush() async {
    if (_flushing) {
      return;
    }
    final patrouilleId = _patrouilleId;
    if (patrouilleId == null) {
      return;
    }
    _flushing = true;
    try {
      while (_queue.isNotEmpty) {
        final fix = _queue.first;
        try {
          await controller.pushPatrouillePosition(
            patrouilleId: patrouilleId,
            latitude: fix.latitude,
            longitude: fix.longitude,
            accuracyM: fix.accuracyM,
          );
          _queue.removeAt(0);
          lastSentAt = DateTime.now();
          error = null;
          notifyListeners();
        } catch (failure) {
          // Keep the fix queued and retry on the next flush.
          error = failure is ApiException
              ? failure.message
              : 'Envoi position impossible';
          notifyListeners();
          break;
        }
      }
    } finally {
      _flushing = false;
    }
  }

  @override
  void dispose() {
    _subscription?.cancel();
    super.dispose();
  }
}
