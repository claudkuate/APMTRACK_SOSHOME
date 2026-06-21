import 'dart:convert';

import '../models.dart';

/// A snapshot of the last successful server sync, persisted for offline reads.
class OfflineSnapshot {
  const OfflineSnapshot({
    this.profile,
    this.interventions = const [],
    this.pvs = const [],
    this.patrouille,
    this.patrouilles = const [],
  });

  final MobileProfile? profile;
  final List<Intervention> interventions;
  final List<Pv> pvs;
  final PatrouilleActive? patrouille;
  final List<Patrouille> patrouilles;

  JsonMap toJson() => {
    'profile': profile?.toJson(),
    'interventions': interventions.map((item) => item.toJson()).toList(),
    'pvs': pvs.map((item) => item.toJson()).toList(),
    'patrouille': patrouille?.toJson(),
    'patrouilles': patrouilles.map((item) => item.toJson()).toList(),
  };

  factory OfflineSnapshot.fromJson(JsonMap json) {
    final rawProfile = json['profile'];
    final rawPatrouille = json['patrouille'];
    return OfflineSnapshot(
      profile: rawProfile is Map<String, dynamic>
          ? MobileProfile.fromJson(rawProfile)
          : null,
      interventions: (json['interventions'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(Intervention.fromJson)
          .toList(),
      pvs: (json['pvs'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(Pv.fromJson)
          .toList(),
      patrouille: rawPatrouille is Map<String, dynamic>
          ? PatrouilleActive.fromJson(rawPatrouille)
          : null,
      patrouilles: (json['patrouilles'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(Patrouille.fromJson)
          .toList(),
    );
  }

  String encode() => jsonEncode(toJson());

  static OfflineSnapshot? decode(String? raw) {
    if (raw == null || raw.isEmpty) {
      return null;
    }
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map<String, dynamic>) {
        return OfflineSnapshot.fromJson(decoded);
      }
    } catch (_) {
      // Corrupt cache is treated as absent.
    }
    return null;
  }
}

/// Result of a PV creation: either synced to the server, or queued as a draft.
class CreatePvOutcome {
  const CreatePvOutcome.synced(Pv this.pv) : draft = null;
  const CreatePvOutcome.queued(PvDraft this.draft) : pv = null;

  final Pv? pv;
  final PvDraft? draft;

  bool get queued => draft != null;
}

enum PvDraftStatus { pending, failed }

/// A PV created offline, awaiting server validation. The server remains the
/// source of truth: number, amount and QR are assigned only on sync.
class PvDraft {
  PvDraft({
    required this.localId,
    required this.payload,
    required this.createdAt,
    this.photos = const [],
    this.interventionName,
    this.amountFcfa,
    this.serverPvId,
    this.status = PvDraftStatus.pending,
    this.error,
  });

  final String localId;
  final CreatePvPayload payload;
  final DateTime createdAt;
  final List<PvDraftPhoto> photos;
  final String? interventionName;
  final int? amountFcfa;
  String? serverPvId;
  PvDraftStatus status;
  String? error;

  JsonMap toJson() => {
    'local_id': localId,
    'payload': payload.toJson(),
    'created_at': createdAt.toIso8601String(),
    'photos': photos.map((photo) => photo.toJson()).toList(),
    'intervention_name': interventionName,
    'amount_fcfa': amountFcfa,
    'server_pv_id': serverPvId,
    'status': status.name,
    'error': error,
  };

  factory PvDraft.fromJson(JsonMap json) {
    return PvDraft(
      localId: readString(json, 'local_id'),
      payload: _payloadFromJson(json['payload'] as JsonMap? ?? const {}),
      createdAt: readDate(json, 'created_at') ?? DateTime.now(),
      photos: (json['photos'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(PvDraftPhoto.fromJson)
          .toList(),
      interventionName: readOptionalString(json, 'intervention_name'),
      amountFcfa: readOptionalInt(json, 'amount_fcfa'),
      serverPvId: readOptionalString(json, 'server_pv_id'),
      status: json['status'] == 'failed'
          ? PvDraftStatus.failed
          : PvDraftStatus.pending,
      error: readOptionalString(json, 'error'),
    );
  }
}

class PvDraftPhoto {
  const PvDraftPhoto({
    required this.path,
    required this.filename,
    required this.contentType,
  });

  final String path;
  final String filename;
  final String contentType;

  JsonMap toJson() => {
    'path': path,
    'filename': filename,
    'content_type': contentType,
  };

  factory PvDraftPhoto.fromJson(JsonMap json) {
    return PvDraftPhoto(
      path: readString(json, 'path'),
      filename: readString(json, 'filename'),
      contentType: readString(json, 'content_type'),
    );
  }
}

CreatePvPayload _payloadFromJson(JsonMap json) => CreatePvPayload(
  interventionId: readString(json, 'intervention_id'),
  interventionIds: (json['intervention_ids'] as List? ?? const [])
      .map((id) => id.toString())
      .where((id) => id.isNotEmpty)
      .toList(),
  subjectType:
      readOptionalString(json, 'subject_type') ??
      PvSubjectTypes.personWithVehicle,
  verbalizedName: readOptionalString(json, 'verbalized_name'),
  verbalizedIdentifier: readOptionalString(json, 'verbalized_identifier'),
  verbalizedFirstName: readOptionalString(json, 'verbalized_first_name'),
  verbalizedLastName: readOptionalString(json, 'verbalized_last_name'),
  verbalizedIdentityType: readOptionalString(json, 'verbalized_identity_type'),
  verbalizedIdentityNumber: readOptionalString(
    json,
    'verbalized_identity_number',
  ),
  verbalizedPhone: readOptionalString(json, 'verbalized_phone'),
  verbalizedAddress: readOptionalString(json, 'verbalized_address'),
  vehiclePlate: readOptionalString(json, 'vehicle_plate'),
  vehicleRegistrationCardNumber: readOptionalString(
    json,
    'vehicle_registration_card_number',
  ),
  vehicleMake: readOptionalString(json, 'vehicle_make'),
  vehicleModel: readOptionalString(json, 'vehicle_model'),
  vehicleColor: readOptionalString(json, 'vehicle_color'),
  vehicleOwnerName: readOptionalString(json, 'vehicle_owner_name'),
  locationDescription: readOptionalString(json, 'location_description'),
  gpsLatitude: readOptionalDouble(json, 'gps_latitude'),
  gpsLongitude: readOptionalDouble(json, 'gps_longitude'),
  notesInternes: readOptionalString(json, 'notes_internes'),
);

String encodeDrafts(List<PvDraft> drafts) =>
    jsonEncode(drafts.map((draft) => draft.toJson()).toList());

List<PvDraft> decodeDrafts(String? raw) {
  if (raw == null || raw.isEmpty) {
    return [];
  }
  try {
    final decoded = jsonDecode(raw);
    if (decoded is List) {
      return decoded
          .whereType<Map<String, dynamic>>()
          .map(PvDraft.fromJson)
          .toList();
    }
  } catch (_) {
    // Corrupt queue is treated as empty.
  }
  return [];
}
