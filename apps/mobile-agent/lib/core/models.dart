typedef JsonMap = Map<String, dynamic>;

String readString(JsonMap json, String key) => json[key]?.toString() ?? '';

String? readOptionalString(JsonMap json, String key) {
  final value = json[key]?.toString().trim();
  return value == null || value.isEmpty ? null : value;
}

int? readOptionalInt(JsonMap json, String key) {
  final value = json[key];
  if (value is int) return value;
  if (value is num) return value.toInt();
  if (value is String) return int.tryParse(value);
  return null;
}

double? readOptionalDouble(JsonMap json, String key) {
  final value = json[key];
  if (value is double) return value;
  if (value is num) return value.toDouble();
  if (value is String) return double.tryParse(value);
  return null;
}

bool readBool(JsonMap json, String key, {bool fallback = false}) {
  final value = json[key];
  if (value is bool) return value;
  if (value is String) return value.toLowerCase() == 'true';
  return fallback;
}

DateTime? readDate(JsonMap json, String key) {
  final value = json[key]?.toString();
  return value == null || value.isEmpty ? null : DateTime.tryParse(value);
}

class Paginated<T> {
  const Paginated({required this.items, required this.total});

  final List<T> items;
  final int total;

  factory Paginated.fromJson(
    JsonMap json,
    T Function(JsonMap json) itemFromJson,
  ) {
    final rawItems = json['items'];
    final items = rawItems is List
        ? rawItems.whereType<Map<String, dynamic>>().map(itemFromJson).toList()
        : <T>[];
    return Paginated(
      items: items,
      total: readOptionalInt(json, 'total') ?? items.length,
    );
  }
}

class AuthSession {
  const AuthSession({
    required this.accessToken,
    required this.refreshToken,
    required this.user,
  });

  final String accessToken;
  final String refreshToken;
  final UserAccount user;

  factory AuthSession.fromJson(JsonMap json) {
    return AuthSession(
      accessToken: readString(json, 'access_token'),
      refreshToken: readString(json, 'refresh_token'),
      user: UserAccount.fromJson(json['user'] as JsonMap),
    );
  }

  JsonMap toJson() => {
    'access_token': accessToken,
    'refresh_token': refreshToken,
    'user': user.toJson(),
  };
}

class UserAccount {
  const UserAccount({
    required this.id,
    required this.email,
    required this.fullName,
    required this.roles,
    required this.active,
    this.communeId,
  });

  final String id;
  final String email;
  final String fullName;
  final String? communeId;
  final List<String> roles;
  final bool active;

  factory UserAccount.fromJson(JsonMap json) {
    return UserAccount(
      id: readString(json, 'id'),
      email: readString(json, 'email'),
      fullName: readString(json, 'full_name'),
      communeId: readOptionalString(json, 'commune_id'),
      roles: (json['roles'] as List? ?? const [])
          .map((role) => role.toString())
          .toList(),
      active: readBool(json, 'active', fallback: true),
    );
  }

  JsonMap toJson() => {
    'id': id,
    'email': email,
    'full_name': fullName,
    'commune_id': communeId,
    'roles': roles,
    'active': active,
  };
}

class Commune {
  const Commune({
    required this.id,
    required this.code,
    required this.nom,
    required this.region,
    required this.departement,
  });

  final String id;
  final String code;
  final String nom;
  final String region;
  final String departement;

  factory Commune.fromJson(JsonMap json) {
    return Commune(
      id: readString(json, 'id'),
      code: readString(json, 'code'),
      nom: readString(json, 'nom'),
      region: readString(json, 'region'),
      departement: readString(json, 'departement'),
    );
  }

  JsonMap toJson() => {
    'id': id,
    'code': code,
    'nom': nom,
    'region': region,
    'departement': departement,
  };
}

class AgentProfile {
  const AgentProfile({
    required this.id,
    required this.matricule,
    required this.fullName,
    required this.communeId,
    required this.status,
    this.datePriseFonction,
    this.photoUrl,
    this.telephone,
    this.email,
  });

  final String id;
  final String matricule;
  final String fullName;
  final String communeId;
  final String status;
  final DateTime? datePriseFonction;
  final String? photoUrl;
  final String? telephone;
  final String? email;

  bool get active => status == 'ACTIF';

  factory AgentProfile.fromJson(JsonMap json) {
    return AgentProfile(
      id: readString(json, 'id'),
      matricule: readString(json, 'matricule'),
      fullName: readString(json, 'full_name'),
      communeId: readString(json, 'commune_id'),
      status: readString(json, 'status'),
      datePriseFonction: readDate(json, 'date_prise_fonction'),
      photoUrl: readOptionalString(json, 'photo_url'),
      telephone: readOptionalString(json, 'telephone'),
      email: readOptionalString(json, 'email'),
    );
  }

  JsonMap toJson() => {
    'id': id,
    'matricule': matricule,
    'full_name': fullName,
    'commune_id': communeId,
    'status': status,
    'date_prise_fonction': datePriseFonction?.toIso8601String(),
    'photo_url': photoUrl,
    'telephone': telephone,
    'email': email,
  };
}

class MobileProfile {
  const MobileProfile({
    required this.user,
    required this.commune,
    required this.agent,
  });

  final UserAccount user;
  final Commune commune;
  final AgentProfile agent;

  factory MobileProfile.fromJson(JsonMap json) {
    return MobileProfile(
      user: UserAccount.fromJson(json['user'] as JsonMap),
      commune: Commune.fromJson(json['commune'] as JsonMap),
      agent: AgentProfile.fromJson(json['agent'] as JsonMap),
    );
  }

  JsonMap toJson() => {
    'user': user.toJson(),
    'commune': commune.toJson(),
    'agent': agent.toJson(),
  };
}

class Intervention {
  const Intervention({
    required this.id,
    required this.nom,
    required this.sujetPaiement,
    required this.active,
    this.categoryId = '',
    this.categoryNom = '',
    this.typeId = '',
    this.typeNom = '',
    this.requiresVehicle = false,
    this.description,
    this.montantFcfa,
    this.delaiPaiementJours,
  });

  final String id;
  final String categoryId;
  final String categoryNom;
  final String typeId;
  final String typeNom;
  final String nom;
  final String? description;
  final bool requiresVehicle;
  final bool sujetPaiement;
  final bool active;
  final int? montantFcfa;
  final int? delaiPaiementJours;

  factory Intervention.fromJson(JsonMap json) {
    return Intervention(
      id: readString(json, 'id'),
      categoryId: readString(json, 'category_id'),
      categoryNom: readString(json, 'category_nom'),
      typeId: readString(json, 'type_id'),
      typeNom: readString(json, 'type_nom'),
      nom: readString(json, 'nom'),
      description: readOptionalString(json, 'description'),
      requiresVehicle: readBool(json, 'requires_vehicle'),
      sujetPaiement: readBool(json, 'sujet_paiement'),
      active: readBool(json, 'active', fallback: true),
      montantFcfa: readOptionalInt(json, 'montant_fcfa'),
      delaiPaiementJours: readOptionalInt(json, 'delai_paiement_jours'),
    );
  }

  JsonMap toJson() => {
    'id': id,
    'category_id': categoryId,
    'category_nom': categoryNom,
    'type_id': typeId,
    'type_nom': typeNom,
    'nom': nom,
    'description': description,
    'requires_vehicle': requiresVehicle,
    'sujet_paiement': sujetPaiement,
    'active': active,
    'montant_fcfa': montantFcfa,
    'delai_paiement_jours': delaiPaiementJours,
  };
}

class Pv {
  const Pv({
    required this.id,
    required this.pvNumber,
    required this.interventionId,
    required this.status,
    required this.createdAt,
    this.interventions = const [],
    this.subjectType = PvSubjectTypes.personWithVehicle,
    this.subjectKind,
    this.raisonSociale,
    this.zoneId,
    this.verbalizedName,
    this.verbalizedIdentifier,
    this.verbalizedFirstName,
    this.verbalizedLastName,
    this.verbalizedIdentityType,
    this.verbalizedIdentityNumber,
    this.verbalizedPhone,
    this.verbalizedAddress,
    this.vehiclePlate,
    this.vehicleRegistrationCardNumber,
    this.vehicleMake,
    this.vehicleModel,
    this.vehicleColor,
    this.vehicleOwnerName,
    this.locationDescription,
    this.gpsLatitude,
    this.gpsLongitude,
    this.amountInitialFcfa,
    this.notesInternes,
  });

  final String id;
  final String pvNumber;
  final String interventionId;
  final List<PvIntervention> interventions;
  final String subjectType;
  final String? subjectKind;
  final String? raisonSociale;
  final String? zoneId;
  final String? verbalizedName;
  final String? verbalizedIdentifier;
  final String? verbalizedFirstName;
  final String? verbalizedLastName;
  final String? verbalizedIdentityType;
  final String? verbalizedIdentityNumber;
  final String? verbalizedPhone;
  final String? verbalizedAddress;
  final String? vehiclePlate;
  final String? vehicleRegistrationCardNumber;
  final String? vehicleMake;
  final String? vehicleModel;
  final String? vehicleColor;
  final String? vehicleOwnerName;
  final String? locationDescription;
  final double? gpsLatitude;
  final double? gpsLongitude;
  final int? amountInitialFcfa;
  final String status;
  final String? notesInternes;
  final DateTime? createdAt;

  bool get canEdit => status != 'PAYE' && status != 'ANNULE';

  String get subjectLabel => switch (subjectType) {
    PvSubjectTypes.personOnly => 'Usager sans vehicule',
    PvSubjectTypes.vehicleOnly => 'Vehicule sans conducteur',
    _ => 'Usager avec vehicule',
  };

  String get infractionsLabel {
    if (interventions.isEmpty) return 'Infraction';
    if (interventions.length == 1) return interventions.first.nom;
    return '${interventions.length} infractions';
  }

  String? get verbalizedDisplayName {
    final composed = [verbalizedFirstName, verbalizedLastName]
        .whereType<String>()
        .map((value) => value.trim())
        .where((value) => value.isNotEmpty)
        .join(' ');
    return composed.isEmpty ? verbalizedName : composed;
  }

  String? get verbalizedIdentityLabel {
    final number = verbalizedIdentityNumber ?? verbalizedIdentifier;
    if (number == null || number.trim().isEmpty) return null;
    final type = verbalizedIdentityType;
    return type == null || type.trim().isEmpty ? number : '$type $number';
  }

  String? get vehicleIdentityLabel {
    if (vehiclePlate != null && vehiclePlate!.trim().isNotEmpty) {
      return vehiclePlate;
    }
    if (vehicleRegistrationCardNumber != null &&
        vehicleRegistrationCardNumber!.trim().isNotEmpty) {
      return 'CG ${vehicleRegistrationCardNumber!}';
    }
    return null;
  }

  factory Pv.fromJson(JsonMap json) {
    return Pv(
      id: readString(json, 'id'),
      pvNumber: readString(json, 'pv_number'),
      interventionId: readString(json, 'intervention_id'),
      interventions: (json['interventions'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(PvIntervention.fromJson)
          .toList(),
      subjectType:
          readOptionalString(json, 'subject_type') ??
          PvSubjectTypes.personWithVehicle,
      subjectKind: readOptionalString(json, 'subject_kind'),
      raisonSociale: readOptionalString(json, 'raison_sociale'),
      zoneId: readOptionalString(json, 'zone_id'),
      verbalizedName: readOptionalString(json, 'verbalized_name'),
      verbalizedIdentifier: readOptionalString(json, 'verbalized_identifier'),
      verbalizedFirstName: readOptionalString(json, 'verbalized_first_name'),
      verbalizedLastName: readOptionalString(json, 'verbalized_last_name'),
      verbalizedIdentityType: readOptionalString(
        json,
        'verbalized_identity_type',
      ),
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
      amountInitialFcfa: readOptionalInt(json, 'amount_initial_fcfa'),
      status: readString(json, 'status'),
      notesInternes: readOptionalString(json, 'notes_internes'),
      createdAt: readDate(json, 'created_at'),
    );
  }

  JsonMap toJson() => {
    'id': id,
    'pv_number': pvNumber,
    'intervention_id': interventionId,
    'interventions': interventions.map((item) => item.toJson()).toList(),
    'subject_type': subjectType,
    'subject_kind': subjectKind,
    'raison_sociale': raisonSociale,
    'zone_id': zoneId,
    'verbalized_name': verbalizedName,
    'verbalized_identifier': verbalizedIdentifier,
    'verbalized_first_name': verbalizedFirstName,
    'verbalized_last_name': verbalizedLastName,
    'verbalized_identity_type': verbalizedIdentityType,
    'verbalized_identity_number': verbalizedIdentityNumber,
    'verbalized_phone': verbalizedPhone,
    'verbalized_address': verbalizedAddress,
    'vehicle_plate': vehiclePlate,
    'vehicle_registration_card_number': vehicleRegistrationCardNumber,
    'vehicle_make': vehicleMake,
    'vehicle_model': vehicleModel,
    'vehicle_color': vehicleColor,
    'vehicle_owner_name': vehicleOwnerName,
    'location_description': locationDescription,
    'gps_latitude': gpsLatitude,
    'gps_longitude': gpsLongitude,
    'amount_initial_fcfa': amountInitialFcfa,
    'status': status,
    'notes_internes': notesInternes,
    'created_at': createdAt?.toIso8601String(),
  };
}

abstract final class PvSubjectTypes {
  static const personOnly = 'PERSON_ONLY';
  static const vehicleOnly = 'VEHICLE_ONLY';
  static const personWithVehicle = 'PERSON_WITH_VEHICLE';
}

class PvIntervention {
  const PvIntervention({
    required this.interventionId,
    required this.orderIndex,
    required this.nom,
    required this.sujetPaiement,
    this.id,
    this.montantFcfa,
    this.delaiPaiementJours,
    this.tauxPenalite,
    this.tauxPenaliteBasisPoints,
  });

  final String? id;
  final String interventionId;
  final int orderIndex;
  final String nom;
  final bool sujetPaiement;
  final int? montantFcfa;
  final int? delaiPaiementJours;
  final double? tauxPenalite;
  final int? tauxPenaliteBasisPoints;

  factory PvIntervention.fromJson(JsonMap json) {
    return PvIntervention(
      id: readOptionalString(json, 'id'),
      interventionId: readString(json, 'intervention_id'),
      orderIndex: readOptionalInt(json, 'order_index') ?? 0,
      nom: readString(json, 'nom'),
      sujetPaiement: readBool(json, 'sujet_paiement'),
      montantFcfa: readOptionalInt(json, 'montant_fcfa'),
      delaiPaiementJours: readOptionalInt(json, 'delai_paiement_jours'),
      tauxPenalite: readOptionalDouble(json, 'taux_penalite'),
      tauxPenaliteBasisPoints: readOptionalInt(
        json,
        'taux_penalite_basis_points',
      ),
    );
  }

  JsonMap toJson() => {
    'id': id,
    'intervention_id': interventionId,
    'order_index': orderIndex,
    'nom': nom,
    'sujet_paiement': sujetPaiement,
    'montant_fcfa': montantFcfa,
    'delai_paiement_jours': delaiPaiementJours,
    'taux_penalite': tauxPenalite,
    'taux_penalite_basis_points': tauxPenaliteBasisPoints,
  };
}

class CreatePvPayload {
  const CreatePvPayload({
    required this.interventionId,
    this.interventionIds = const [],
    this.subjectType = PvSubjectTypes.personWithVehicle,
    this.subjectKind,
    this.raisonSociale,
    this.verbalizedName,
    this.verbalizedIdentifier,
    this.verbalizedFirstName,
    this.verbalizedLastName,
    this.verbalizedIdentityType,
    this.verbalizedIdentityNumber,
    this.verbalizedPhone,
    this.verbalizedAddress,
    this.vehiclePlate,
    this.vehicleRegistrationCardNumber,
    this.vehicleMake,
    this.vehicleModel,
    this.vehicleColor,
    this.vehicleOwnerName,
    this.locationDescription,
    this.gpsLatitude,
    this.gpsLongitude,
    this.notesInternes,
  });

  final String interventionId;
  final List<String> interventionIds;
  final String subjectType;
  final String? subjectKind;
  final String? raisonSociale;
  final String? verbalizedName;
  final String? verbalizedIdentifier;
  final String? verbalizedFirstName;
  final String? verbalizedLastName;
  final String? verbalizedIdentityType;
  final String? verbalizedIdentityNumber;
  final String? verbalizedPhone;
  final String? verbalizedAddress;
  final String? vehiclePlate;
  final String? vehicleRegistrationCardNumber;
  final String? vehicleMake;
  final String? vehicleModel;
  final String? vehicleColor;
  final String? vehicleOwnerName;
  final String? locationDescription;
  final double? gpsLatitude;
  final double? gpsLongitude;
  final String? notesInternes;

  JsonMap toJson() => {
    'intervention_id': interventionId,
    'intervention_ids': interventionIds.isEmpty
        ? [interventionId]
        : interventionIds,
    'subject_type': subjectType,
    'subject_kind': subjectKind,
    'raison_sociale': raisonSociale,
    'verbalized_name': verbalizedName,
    'verbalized_identifier': verbalizedIdentifier,
    'verbalized_first_name': verbalizedFirstName,
    'verbalized_last_name': verbalizedLastName,
    'verbalized_identity_type': verbalizedIdentityType,
    'verbalized_identity_number': verbalizedIdentityNumber,
    'verbalized_phone': verbalizedPhone,
    'verbalized_address': verbalizedAddress,
    'vehicle_plate': vehiclePlate,
    'vehicle_registration_card_number': vehicleRegistrationCardNumber,
    'vehicle_make': vehicleMake,
    'vehicle_model': vehicleModel,
    'vehicle_color': vehicleColor,
    'vehicle_owner_name': vehicleOwnerName,
    'location_description': locationDescription,
    'gps_latitude': gpsLatitude,
    'gps_longitude': gpsLongitude,
    'notes_internes': notesInternes,
  };
}

class PvPhoto {
  const PvPhoto({
    required this.id,
    required this.pvId,
    required this.contentType,
    required this.sizeBytes,
    this.createdAt,
  });

  final String id;
  final String pvId;
  final String contentType;
  final int sizeBytes;
  final DateTime? createdAt;

  factory PvPhoto.fromJson(JsonMap json) {
    return PvPhoto(
      id: readString(json, 'id'),
      pvId: readString(json, 'pv_id'),
      contentType: readString(json, 'content_type'),
      sizeBytes: readOptionalInt(json, 'size_bytes') ?? 0,
      createdAt: readDate(json, 'created_at'),
    );
  }
}

class PvPublic {
  const PvPublic({
    required this.pvNumber,
    required this.status,
    this.amountInitialFcfa,
    this.createdAt,
  });

  final String pvNumber;
  final String status;
  final int? amountInitialFcfa;
  final DateTime? createdAt;

  factory PvPublic.fromJson(JsonMap json) {
    return PvPublic(
      pvNumber: readString(json, 'pv_number'),
      status: readString(json, 'status'),
      amountInitialFcfa: readOptionalInt(json, 'amount_initial_fcfa'),
      createdAt: readDate(json, 'created_at'),
    );
  }
}

class Patrouille {
  const Patrouille({
    required this.id,
    required this.nom,
    required this.status,
    this.description,
    this.zoneId,
    this.dateDebut,
    this.dateFin,
    this.dateDebutPrevue,
    this.dateFinPrevue,
  });

  final String id;
  final String nom;
  final String status;
  final String? description;
  final String? zoneId;
  final DateTime? dateDebut;
  final DateTime? dateFin;
  final DateTime? dateDebutPrevue;
  final DateTime? dateFinPrevue;

  factory Patrouille.fromJson(JsonMap json) {
    return Patrouille(
      id: readString(json, 'id'),
      nom: readString(json, 'nom'),
      status: readString(json, 'status'),
      description: readOptionalString(json, 'description'),
      zoneId: readOptionalString(json, 'zone_id'),
      dateDebut: readDate(json, 'date_debut'),
      dateFin: readDate(json, 'date_fin'),
      dateDebutPrevue: readDate(json, 'date_debut_prevue'),
      dateFinPrevue: readDate(json, 'date_fin_prevue'),
    );
  }

  JsonMap toJson() => {
    'id': id,
    'nom': nom,
    'status': status,
    'description': description,
    'zone_id': zoneId,
    'date_debut': dateDebut?.toIso8601String(),
    'date_fin': dateFin?.toIso8601String(),
    'date_debut_prevue': dateDebutPrevue?.toIso8601String(),
    'date_fin_prevue': dateFinPrevue?.toIso8601String(),
  };
}

class PatrouilleMember {
  const PatrouilleMember({
    required this.agentId,
    required this.matricule,
    required this.fullName,
    required this.rolePatrouille,
  });

  final String agentId;
  final String matricule;
  final String fullName;
  final String rolePatrouille;

  factory PatrouilleMember.fromJson(JsonMap json) {
    return PatrouilleMember(
      agentId: readString(json, 'agent_id'),
      matricule: readString(json, 'matricule'),
      fullName: readString(json, 'full_name'),
      rolePatrouille: readString(json, 'role_patrouille'),
    );
  }

  JsonMap toJson() => {
    'agent_id': agentId,
    'matricule': matricule,
    'full_name': fullName,
    'role_patrouille': rolePatrouille,
  };
}

class PatrouilleActive {
  const PatrouilleActive({this.patrouille, required this.agents});

  final Patrouille? patrouille;
  final List<PatrouilleMember> agents;

  factory PatrouilleActive.fromJson(JsonMap json) {
    final rawPatrouille = json['patrouille'];
    return PatrouilleActive(
      patrouille: rawPatrouille is Map<String, dynamic>
          ? Patrouille.fromJson(rawPatrouille)
          : null,
      agents: (json['agents'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(PatrouilleMember.fromJson)
          .toList(),
    );
  }

  JsonMap toJson() => {
    'patrouille': patrouille?.toJson(),
    'agents': agents.map((agent) => agent.toJson()).toList(),
  };
}
