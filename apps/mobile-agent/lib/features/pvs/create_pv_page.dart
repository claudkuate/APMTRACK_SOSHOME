import 'dart:io';

import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart';
import 'package:image_picker/image_picker.dart';
import 'package:path_provider/path_provider.dart';

import '../../core/api/api_client.dart';
import '../../core/auth/session_controller.dart';
import '../../core/models.dart';
import '../../core/offline/offline_models.dart';
import '../../core/theme.dart';
import '../../core/ui/common.dart';
import 'pv_detail_page.dart';

class CreatePvPage extends StatefulWidget {
  const CreatePvPage({super.key, required this.controller, this.initialPv});

  final SessionController controller;
  final Pv? initialPv;

  @override
  State<CreatePvPage> createState() => _CreatePvPageState();
}

class _CreatePvPageState extends State<CreatePvPage> {
  static const _stepLabels = [
    'Infractions',
    'Sujet',
    'Localisation',
    'Preuves',
    'Revue',
  ];

  final _picker = ImagePicker();
  final _firstNameController = TextEditingController();
  final _lastNameController = TextEditingController();
  final _raisonSocialeController = TextEditingController();
  final _identityNumberController = TextEditingController();
  final _phoneController = TextEditingController();
  final _addressController = TextEditingController();
  final _plateController = TextEditingController();
  final _registrationCardController = TextEditingController();
  final _vehicleMakeController = TextEditingController();
  final _vehicleModelController = TextEditingController();
  final _vehicleColorController = TextEditingController();
  final _vehicleOwnerController = TextEditingController();
  final _locationController = TextEditingController();
  final _notesController = TextEditingController();
  final _interventionSearchController = TextEditingController();

  int _step = 0;
  String? _selectedCategoryId;
  String? _selectedTypeId;
  String? _identityType;
  // Type de personne du contrevenant : 'PHYSIQUE' (défaut) ou 'MORALE'.
  String _subjectKind = 'PHYSIQUE';
  final Set<String> _selectedInterventionIds = {};
  final List<PvDraftPhoto> _photos = [];
  Position? _position;
  bool _locating = false;
  bool _submitting = false;
  String? _error;

  bool get _editing => widget.initialPv != null;

  static const _identityTypeOptions = [
    'CNI',
    'PASSEPORT',
    'PERMIS_CONDUIRE',
    'CARTE_SEJOUR',
    'NIU',
    'AUTRE',
  ];

  static const _searchAccentMap = {
    '\u00e0': 'a',
    '\u00e1': 'a',
    '\u00e2': 'a',
    '\u00e3': 'a',
    '\u00e4': 'a',
    '\u00e5': 'a',
    '\u00e6': 'ae',
    '\u00e7': 'c',
    '\u00e8': 'e',
    '\u00e9': 'e',
    '\u00ea': 'e',
    '\u00eb': 'e',
    '\u00ec': 'i',
    '\u00ed': 'i',
    '\u00ee': 'i',
    '\u00ef': 'i',
    '\u00f1': 'n',
    '\u00f2': 'o',
    '\u00f3': 'o',
    '\u00f4': 'o',
    '\u00f5': 'o',
    '\u00f6': 'o',
    '\u0153': 'oe',
    '\u00f9': 'u',
    '\u00fa': 'u',
    '\u00fb': 'u',
    '\u00fc': 'u',
    '\u00fd': 'y',
    '\u00ff': 'y',
  };

  @override
  void initState() {
    super.initState();
    final pv = widget.initialPv;
    if (pv == null) {
      if (widget.controller.interventions.isNotEmpty) {
        _selectedInterventionIds.add(widget.controller.interventions.first.id);
      }
      return;
    }
    _subjectKind = pv.subjectKind ?? 'PHYSIQUE';
    _raisonSocialeController.text =
        pv.raisonSociale ?? (_subjectKind == 'MORALE' ? pv.verbalizedName ?? '' : '');
    _firstNameController.text = pv.verbalizedFirstName ?? '';
    _lastNameController.text = pv.verbalizedLastName ?? pv.verbalizedName ?? '';
    _identityType = pv.verbalizedIdentityType;
    _identityNumberController.text =
        pv.verbalizedIdentityNumber ?? pv.verbalizedIdentifier ?? '';
    _phoneController.text = pv.verbalizedPhone ?? '';
    _addressController.text = pv.verbalizedAddress ?? '';
    _plateController.text = pv.vehiclePlate ?? '';
    _registrationCardController.text = pv.vehicleRegistrationCardNumber ?? '';
    _vehicleMakeController.text = pv.vehicleMake ?? '';
    _vehicleModelController.text = pv.vehicleModel ?? '';
    _vehicleColorController.text = pv.vehicleColor ?? '';
    _vehicleOwnerController.text = pv.vehicleOwnerName ?? '';
    _locationController.text = pv.locationDescription ?? '';
    _notesController.text = pv.notesInternes ?? '';
    if (pv.gpsLatitude != null && pv.gpsLongitude != null) {
      _position = Position(
        latitude: pv.gpsLatitude!,
        longitude: pv.gpsLongitude!,
        timestamp: DateTime.now(),
        accuracy: 0,
        altitude: 0,
        heading: 0,
        speed: 0,
        speedAccuracy: 0,
        altitudeAccuracy: 0,
        headingAccuracy: 0,
      );
    }
    final ids = pv.interventions.map((item) => item.interventionId);
    _selectedInterventionIds.addAll(ids);
    if (_selectedInterventionIds.isEmpty) {
      _selectedInterventionIds.add(pv.interventionId);
    }
  }

  @override
  void dispose() {
    _firstNameController.dispose();
    _lastNameController.dispose();
    _raisonSocialeController.dispose();
    _identityNumberController.dispose();
    _phoneController.dispose();
    _addressController.dispose();
    _plateController.dispose();
    _registrationCardController.dispose();
    _vehicleMakeController.dispose();
    _vehicleModelController.dispose();
    _vehicleColorController.dispose();
    _vehicleOwnerController.dispose();
    _locationController.dispose();
    _notesController.dispose();
    _interventionSearchController.dispose();
    super.dispose();
  }

  List<Intervention> get _selectedInterventions {
    return widget.controller.interventions
        .where((item) => _selectedInterventionIds.contains(item.id))
        .toList();
  }

  bool get _selectedRequiresVehicle {
    return _selectedInterventions.any((item) => item.requiresVehicle);
  }

  bool get _hasVehicleInput {
    return _clean(_plateController.text) != null ||
        _clean(_registrationCardController.text) != null ||
        _clean(_vehicleMakeController.text) != null ||
        _clean(_vehicleModelController.text) != null ||
        _clean(_vehicleColorController.text) != null ||
        _clean(_vehicleOwnerController.text) != null;
  }

  bool get _vehicleInvolved {
    return _selectedRequiresVehicle || _hasVehicleInput;
  }

  List<Intervention> get _categoryOptions {
    final seen = <String>{};
    return widget.controller.interventions
        .where((item) => seen.add(item.categoryId))
        .toList();
  }

  List<Intervention> get _typeOptions {
    final seen = <String>{};
    return widget.controller.interventions
        .where(
          (item) =>
              _selectedCategoryId == null || item.categoryId == _selectedCategoryId,
        )
        .where((item) => seen.add(item.typeId))
        .toList();
  }

  int? get _totalFcfa {
    var total = 0;
    for (final item in _selectedInterventions) {
      if (item.sujetPaiement) {
        total += item.montantFcfa ?? 0;
      }
    }
    return total == 0 ? null : total;
  }

  List<Intervention> get _filteredInterventions {
    final query = _normalizeForSearch(_interventionSearchController.text);
    return widget.controller.interventions
        .where(
          (item) =>
              _selectedCategoryId == null || item.categoryId == _selectedCategoryId,
        )
        .where((item) => _selectedTypeId == null || item.typeId == _selectedTypeId)
        .where((item) => _interventionSearchText(item).contains(query))
        .toList();
  }

  Future<void> _captureGps() async {
    setState(() {
      _locating = true;
      _error = null;
    });
    try {
      final serviceEnabled = await Geolocator.isLocationServiceEnabled();
      if (!serviceEnabled) {
        throw ApiException('GPS desactive sur le telephone');
      }
      var permission = await Geolocator.checkPermission();
      if (permission == LocationPermission.denied) {
        permission = await Geolocator.requestPermission();
      }
      if (permission == LocationPermission.denied ||
          permission == LocationPermission.deniedForever) {
        throw ApiException('GPS refuse');
      }
      final position = await Geolocator.getCurrentPosition(
        locationSettings: const LocationSettings(
          accuracy: LocationAccuracy.high,
          timeLimit: Duration(seconds: 12),
        ),
      );
      setState(() => _position = position);
    } on ApiException catch (error) {
      setState(() => _error = error.message);
    } catch (_) {
      setState(() => _error = 'Capture GPS impossible');
    } finally {
      if (mounted) {
        setState(() => _locating = false);
      }
    }
  }

  Future<void> _pickPhoto(ImageSource source) async {
    setState(() => _error = null);
    try {
      final file = await _picker.pickImage(
        source: source,
        imageQuality: 70,
        maxWidth: 1600,
      );
      if (file == null) return;
      final directory = Directory(
        '${(await getApplicationDocumentsDirectory()).path}${Platform.pathSeparator}pv-draft-photos',
      );
      if (!await directory.exists()) {
        await directory.create(recursive: true);
      }
      final safeName = file.name.replaceAll(RegExp(r'[^A-Za-z0-9._-]'), '_');
      final target = File(
        '${directory.path}${Platform.pathSeparator}${DateTime.now().microsecondsSinceEpoch}-$safeName',
      );
      await File(file.path).copy(target.path);
      setState(() {
        _photos.add(
          PvDraftPhoto(
            path: target.path,
            filename: safeName,
            contentType: file.mimeType ?? _guessMime(file.name),
          ),
        );
      });
    } catch (_) {
      setState(() => _error = 'Ajout de la photo impossible');
    }
  }

  Future<void> _removePhoto(PvDraftPhoto photo) async {
    setState(() => _photos.remove(photo));
    try {
      final file = File(photo.path);
      if (await file.exists()) {
        await file.delete();
      }
    } catch (_) {}
  }

  Future<void> _submit() async {
    final invalidStep = _firstInvalidStep();
    if (invalidStep != null) {
      setState(() => _step = invalidStep);
      return;
    }
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      final payload = _payload();
      if (_editing) {
        final pv = await widget.controller.updatePv(
          widget.initialPv!.id,
          payload,
        );
        // Le PV est déjà mis à jour côté serveur : un échec d'upload photo ne
        // doit pas être présenté comme un échec de la mise à jour elle-même.
        var photosMessage = '';
        try {
          await _uploadLocalPhotos(pv.id);
        } catch (_) {
          photosMessage =
              ' (photos non envoyees : reessayez depuis la fiche PV)';
        }
        if (!mounted) return;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              'PV officiel mis a jour : ${pv.pvNumber}$photosMessage',
            ),
          ),
        );
        Navigator.of(context).pushReplacement(
          MaterialPageRoute(
            builder: (_) => PvDetailPage(controller: widget.controller, pv: pv),
          ),
        );
        return;
      }

      final outcome = await widget.controller.createPv(
        payload,
        photos: _photos,
      );
      if (!mounted) return;
      if (outcome.queued) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text(
              'Hors-ligne : brouillon local enregistre. Numero, montant officiel et QR seront attribues apres synchronisation serveur.',
            ),
          ),
        );
        Navigator.of(context).pop(true);
        return;
      }
      final pv = outcome.pv!;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('PV officiel cree par le serveur : ${pv.pvNumber}'),
        ),
      );
      Navigator.of(context).pushReplacement(
        MaterialPageRoute(
          builder: (_) => PvDetailPage(controller: widget.controller, pv: pv),
        ),
      );
    } on ApiException catch (error) {
      setState(() => _error = error.message);
    } catch (_) {
      setState(
        () => _error = _editing
            ? 'Mise a jour impossible'
            : 'Enregistrement impossible',
      );
    } finally {
      if (mounted) {
        setState(() => _submitting = false);
      }
    }
  }

  Future<void> _uploadLocalPhotos(String pvId) async {
    for (final photo in List<PvDraftPhoto>.from(_photos)) {
      final file = File(photo.path);
      if (!await file.exists()) continue;
      await widget.controller.uploadPvPhoto(
        pvId,
        bytes: await file.readAsBytes(),
        filename: photo.filename,
        contentType: photo.contentType,
      );
      await _removePhoto(photo);
    }
  }

  CreatePvPayload _payload() {
    final ids = _selectedInterventions.map((item) => item.id).toList();
    final hasVehicle = _vehicleInvolved;
    final isMorale = _subjectKind == 'MORALE';
    final identityNumber = _clean(_identityNumberController.text);
    final raisonSociale = _clean(_raisonSocialeController.text);
    // En personne morale, la raison sociale tient lieu de nom du contrevenant ;
    // Nom/Prénom ne sont pas saisis.
    final firstName = isMorale ? null : _clean(_firstNameController.text);
    final lastName = isMorale ? null : _clean(_lastNameController.text);
    final name = isMorale
        ? raisonSociale
        : [firstName, lastName].whereType<String>().join(' ');
    return CreatePvPayload(
      interventionId: ids.first,
      interventionIds: ids,
      subjectType: hasVehicle
          ? PvSubjectTypes.personWithVehicle
          : PvSubjectTypes.personOnly,
      subjectKind: isMorale ? 'MORALE' : null,
      raisonSociale: isMorale ? raisonSociale : null,
      verbalizedName: name == null || name.isEmpty ? null : name,
      verbalizedIdentifier: identityNumber,
      verbalizedFirstName: firstName,
      verbalizedLastName: lastName,
      verbalizedIdentityType: identityNumber == null ? null : _identityType,
      verbalizedIdentityNumber: identityNumber,
      verbalizedPhone: _clean(_phoneController.text),
      verbalizedAddress: _clean(_addressController.text),
      vehiclePlate: hasVehicle
          ? _clean(_plateController.text)?.toUpperCase()
          : null,
      vehicleRegistrationCardNumber: hasVehicle
          ? _clean(_registrationCardController.text)?.toUpperCase()
          : null,
      vehicleMake: hasVehicle ? _clean(_vehicleMakeController.text) : null,
      vehicleModel: hasVehicle ? _clean(_vehicleModelController.text) : null,
      vehicleColor: hasVehicle ? _clean(_vehicleColorController.text) : null,
      vehicleOwnerName: hasVehicle
          ? _clean(_vehicleOwnerController.text)
          : null,
      locationDescription: _clean(_locationController.text),
      gpsLatitude: _position?.latitude,
      gpsLongitude: _position?.longitude,
      notesInternes: _clean(_notesController.text),
    );
  }

  int? _firstInvalidStep() {
    if (_selectedInterventionIds.isEmpty) {
      _setError('Selectionnez au moins une infraction');
      return 0;
    }
    final isMorale = _subjectKind == 'MORALE';
    final hasLastName = _clean(_lastNameController.text) != null;
    final hasRaisonSociale = _clean(_raisonSocialeController.text) != null;
    final hasPhone = _clean(_phoneController.text) != null;
    final hasIdentityNumber = _clean(_identityNumberController.text) != null;
    final hasIdentityType = _identityType != null && _identityType!.isNotEmpty;
    final hasVehicle =
        _clean(_plateController.text) != null ||
        _clean(_registrationCardController.text) != null;
    if (isMorale && !hasRaisonSociale) {
      _setError('Raison sociale requise');
      return 1;
    }
    if (!isMorale && !hasLastName) {
      _setError('Nom du contrevenant requis');
      return 1;
    }
    if (!hasPhone) {
      _setError('Telephone du contrevenant requis');
      return 1;
    }
    if (hasIdentityNumber && !hasIdentityType) {
      _setError('Type d identite requis avec le numero');
      return 1;
    }
    // Miroir de `has_vehicle_any` côté serveur : dès qu'un champ véhicule est
    // rempli (ou que l'infraction l'exige), la plaque ou la carte grise est
    // obligatoire — sinon le serveur rejettera le PV (erreur tardive en ligne,
    // brouillon voué à échouer hors-ligne).
    if (_vehicleInvolved && !hasVehicle) {
      _setError('Plaque ou carte grise requise');
      return 1;
    }
    if (_clean(_locationController.text) == null) {
      _setError('Lieu requis');
      return 2;
    }
    setState(() => _error = null);
    return null;
  }

  void _setError(String message) {
    setState(() => _error = message);
  }

  String? _clean(String value) {
    final trimmed = value.trim();
    return trimmed.isEmpty ? null : trimmed;
  }

  String _guessMime(String name) {
    final lower = name.toLowerCase();
    if (lower.endsWith('.png')) return 'image/png';
    if (lower.endsWith('.webp')) return 'image/webp';
    if (lower.endsWith('.heic')) return 'image/heic';
    return 'image/jpeg';
  }

  String _summaryText(Iterable<String?> values) {
    final value = values.whereType<String>().join(' - ');
    return value.isEmpty ? '-' : value;
  }

  String _interventionSearchText(Intervention item) {
    return _normalizeForSearch(
      [
        item.categoryNom,
        item.typeNom,
        item.nom,
        item.description,
        item.montantFcfa?.toString(),
        formatFcfa(item.montantFcfa),
      ].whereType<String>().join(' '),
    );
  }

  String _normalizeForSearch(String value) {
    final buffer = StringBuffer();
    for (final codePoint in value.toLowerCase().trim().runes) {
      final char = String.fromCharCode(codePoint);
      buffer.write(_searchAccentMap[char] ?? char);
    }
    return buffer
        .toString()
        .replaceAll(RegExp('[\\s\u00a0\u202f]+'), ' ')
        .trim();
  }

  void _next() {
    final invalidStep = _firstInvalidStep();
    if (invalidStep != null && invalidStep <= _step) {
      setState(() => _step = invalidStep);
      return;
    }
    setState(() {
      _error = null;
      _step = (_step + 1).clamp(0, _stepLabels.length - 1);
    });
  }

  @override
  Widget build(BuildContext context) {
    final title = _editing ? 'Modifier PV officiel' : 'Nouvelle saisie PV';
    return Scaffold(
      appBar: AppBar(title: Text(title)),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 96),
        children: [
          SectionPanel(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'Etape ${_step + 1}/${_stepLabels.length} - ${_stepLabels[_step]}',
                  style: const TextStyle(fontWeight: FontWeight.w900),
                ),
                const SizedBox(height: 10),
                LinearProgressIndicator(
                  value: (_step + 1) / _stepLabels.length,
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
          _buildStep(),
          if (_error != null) ...[
            const SizedBox(height: 12),
            Text(
              _error!,
              style: const TextStyle(
                color: apmRed,
                fontWeight: FontWeight.w800,
              ),
            ),
          ],
        ],
      ),
      bottomNavigationBar: SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Row(
            children: [
              if (_step > 0)
                Expanded(
                  child: OutlinedButton.icon(
                    onPressed: _submitting
                        ? null
                        : () => setState(() => _step--),
                    icon: const Icon(Icons.chevron_left),
                    label: const Text('Retour'),
                  ),
                ),
              if (_step > 0) const SizedBox(width: 10),
              Expanded(
                child: FilledButton.icon(
                  onPressed: _submitting
                      ? null
                      : _step == _stepLabels.length - 1
                      ? _submit
                      : _next,
                  icon: _submitting
                      ? const SizedBox(
                          width: 18,
                          height: 18,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : Icon(
                          _step == _stepLabels.length - 1
                              ? Icons.cloud_done_outlined
                              : Icons.chevron_right,
                        ),
                  label: Text(
                    _submitting
                        ? 'Enregistrement...'
                        : _step == _stepLabels.length - 1
                        ? (_editing ? 'Mettre a jour' : 'Enregistrer')
                        : 'Continuer',
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildStep() {
    return switch (_step) {
      0 => _buildInfractionsStep(),
      1 => _buildSubjectStep(),
      2 => _buildLocationStep(),
      3 => _buildPhotosStep(),
      _ => _buildReviewStep(),
    };
  }

  Future<void> _retryReferentiel() async {
    await widget.controller.refreshData();
    if (!mounted) return;
    setState(() {
      if (_selectedInterventionIds.isEmpty &&
          widget.controller.interventions.isNotEmpty) {
        _selectedInterventionIds.add(widget.controller.interventions.first.id);
      }
    });
  }

  Widget _buildInfractionsStep() {
    final interventions = widget.controller.interventions;
    final filteredInterventions = _filteredInterventions;
    final hasSearch = _interventionSearchController.text.trim().isNotEmpty;
    return SectionPanel(
      padding: EdgeInsets.zero,
      child: Column(
        children: [
          if (interventions.isEmpty)
            Padding(
              padding: const EdgeInsets.all(16),
              child: widget.controller.offline
                  ? Column(
                      children: [
                        const EmptyState(
                          title: 'Referentiel non charge',
                          message:
                              'Connexion au serveur impossible (hors-ligne ou erreur). Reessayez une fois en ligne.',
                          icon: Icons.cloud_off_outlined,
                        ),
                        const SizedBox(height: 12),
                        FilledButton.icon(
                          onPressed: widget.controller.loadingData
                              ? null
                              : _retryReferentiel,
                          icon: widget.controller.loadingData
                              ? const SizedBox(
                                  width: 18,
                                  height: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : const Icon(Icons.refresh),
                          label: Text(
                            widget.controller.loadingData
                                ? 'Chargement...'
                                : 'Reessayer',
                          ),
                        ),
                      ],
                    )
                  : const EmptyState(
                      title: 'Aucune infraction',
                      message: 'Le referentiel mobile est vide.',
                      icon: Icons.rule_folder_outlined,
                    ),
            )
          else ...[
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
              child: Column(
                children: [
                  DropdownButtonFormField<String?>(
                    initialValue: _selectedCategoryId,
                    decoration: const InputDecoration(
                      labelText: 'Categorie',
                      prefixIcon: Icon(Icons.account_tree_outlined),
                    ),
                    items: [
                      const DropdownMenuItem(value: null, child: Text('Toutes')),
                      ..._categoryOptions.map(
                        (item) => DropdownMenuItem(
                          value: item.categoryId,
                          child: Text(item.categoryNom),
                        ),
                      ),
                    ],
                    onChanged: (value) {
                      setState(() {
                        _selectedCategoryId = value;
                        _selectedTypeId = null;
                      });
                    },
                  ),
                  const SizedBox(height: 10),
                  DropdownButtonFormField<String?>(
                    initialValue: _selectedTypeId,
                    decoration: const InputDecoration(
                      labelText: 'Type',
                      prefixIcon: Icon(Icons.rule_folder_outlined),
                    ),
                    items: [
                      const DropdownMenuItem(value: null, child: Text('Tous')),
                      ..._typeOptions.map(
                        (item) => DropdownMenuItem(
                          value: item.typeId,
                          child: Text(item.typeNom),
                        ),
                      ),
                    ],
                    onChanged: (value) => setState(() => _selectedTypeId = value),
                  ),
                  const SizedBox(height: 10),
                  TextField(
                    key: const Key('intervention-search-field'),
                    controller: _interventionSearchController,
                    textInputAction: TextInputAction.search,
                    onChanged: (_) => setState(() {}),
                    decoration: InputDecoration(
                      labelText: 'Rechercher une infraction',
                      prefixIcon: const Icon(Icons.search),
                      suffixIcon: hasSearch
                          ? IconButton(
                              tooltip: 'Effacer la recherche',
                              icon: const Icon(Icons.clear),
                              onPressed: () {
                                setState(_interventionSearchController.clear);
                              },
                            )
                          : null,
                    ),
                  ),
                ],
              ),
            ),
            if (_selectedRequiresVehicle)
              const Padding(
                padding: EdgeInsets.fromLTRB(16, 0, 16, 8),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: StatusPill(status: 'VEHICULE REQUIS'),
                ),
              ),
            if (filteredInterventions.isEmpty)
              const Padding(
                padding: EdgeInsets.fromLTRB(16, 20, 16, 24),
                child: Column(
                  children: [
                    Icon(Icons.search_off_outlined, color: apmMuted, size: 32),
                    SizedBox(height: 10),
                    Text(
                      'Aucun resultat',
                      style: TextStyle(fontWeight: FontWeight.w900),
                    ),
                    SizedBox(height: 6),
                    Text(
                      'Aucune infraction ne correspond a cette recherche.',
                      textAlign: TextAlign.center,
                      style: TextStyle(color: apmMuted),
                    ),
                  ],
                ),
              )
            else
              for (final item in filteredInterventions)
                CheckboxListTile(
                  value: _selectedInterventionIds.contains(item.id),
                  onChanged: (selected) {
                    setState(() {
                      if (selected == true) {
                        _selectedInterventionIds.add(item.id);
                      } else {
                        _selectedInterventionIds.remove(item.id);
                      }
                    });
                  },
                  title: Text(
                    item.nom,
                    style: const TextStyle(fontWeight: FontWeight.w800),
                  ),
                  subtitle: Text(
                    '${item.categoryNom} - ${item.typeNom} - ${formatFcfa(item.montantFcfa)}',
                  ),
                ),
          ],
          const Divider(height: 1),
          ListTile(
            title: const Text('Montant indicatif'),
            trailing: Text(
              formatFcfa(_totalFcfa),
              style: const TextStyle(fontWeight: FontWeight.w900),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildSubjectStep() {
    return SectionPanel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _buildPersonFields(),
          const SizedBox(height: 16),
          _buildVehicleFields(),
        ],
      ),
    );
  }

  Widget _buildPersonFields() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const Text(
          'Contrevenant',
          style: TextStyle(fontWeight: FontWeight.w900),
        ),
        const SizedBox(height: 12),
        SegmentedButton<String>(
          segments: const [
            ButtonSegment(value: 'PHYSIQUE', label: Text('Personne physique')),
            ButtonSegment(value: 'MORALE', label: Text('Personne morale')),
          ],
          selected: {_subjectKind},
          showSelectedIcon: false,
          onSelectionChanged: (selection) =>
              setState(() => _subjectKind = selection.first),
        ),
        const SizedBox(height: 12),
        if (_subjectKind == 'MORALE')
          TextField(
            controller: _raisonSocialeController,
            textCapitalization: TextCapitalization.words,
            decoration: const InputDecoration(
              labelText: 'Raison sociale',
              hintText: 'Ex. Ets CAMERAMAN',
              prefixIcon: Icon(Icons.business_outlined),
            ),
          )
        else
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _lastNameController,
                  textCapitalization: TextCapitalization.words,
                  decoration: const InputDecoration(
                    labelText: 'Nom',
                    prefixIcon: Icon(Icons.person_outline),
                  ),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: TextField(
                  controller: _firstNameController,
                  textCapitalization: TextCapitalization.words,
                  decoration: const InputDecoration(labelText: 'Prenom'),
                ),
              ),
            ],
          ),
        const SizedBox(height: 12),
        DropdownButtonFormField<String>(
          initialValue: _identityType,
          decoration: const InputDecoration(
            labelText: 'Type d identite',
            prefixIcon: Icon(Icons.badge_outlined),
          ),
          items: [
            const DropdownMenuItem(value: null, child: Text('Choisir...')),
            ..._identityTypeOptions.map(
              (value) => DropdownMenuItem(value: value, child: Text(value)),
            ),
          ],
          onChanged: (value) => setState(() => _identityType = value),
        ),
        const SizedBox(height: 12),
        TextField(
          controller: _identityNumberController,
          textCapitalization: TextCapitalization.characters,
          decoration: const InputDecoration(
            labelText: 'Numero d identite',
            prefixIcon: Icon(Icons.confirmation_number_outlined),
          ),
        ),
        const SizedBox(height: 12),
        TextField(
          controller: _phoneController,
          keyboardType: TextInputType.phone,
          decoration: const InputDecoration(
            labelText: 'Telephone',
            prefixIcon: Icon(Icons.call_outlined),
          ),
        ),
        const SizedBox(height: 12),
        TextField(
          controller: _addressController,
          decoration: const InputDecoration(
            labelText: 'Adresse',
            prefixIcon: Icon(Icons.home_outlined),
          ),
        ),
      ],
    );
  }

  Widget _buildVehicleFields() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            const Text(
              'Vehicule',
              style: TextStyle(fontWeight: FontWeight.w900),
            ),
            const SizedBox(width: 8),
            Text(
              _selectedRequiresVehicle ? '(requis)' : '(optionnel)',
              style: const TextStyle(color: apmMuted, fontSize: 12),
            ),
          ],
        ),
        const SizedBox(height: 12),
        TextField(
          controller: _plateController,
          textCapitalization: TextCapitalization.characters,
          decoration: const InputDecoration(
            labelText: 'Plaque vehicule',
            prefixIcon: Icon(Icons.directions_car_outlined),
          ),
        ),
        const SizedBox(height: 12),
        TextField(
          controller: _registrationCardController,
          textCapitalization: TextCapitalization.characters,
          decoration: const InputDecoration(
            labelText: 'Numero carte grise',
            prefixIcon: Icon(Icons.article_outlined),
          ),
        ),
        const SizedBox(height: 12),
        Row(
          children: [
            Expanded(
              child: TextField(
                controller: _vehicleMakeController,
                textCapitalization: TextCapitalization.words,
                decoration: const InputDecoration(labelText: 'Marque'),
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: TextField(
                controller: _vehicleModelController,
                textCapitalization: TextCapitalization.words,
                decoration: const InputDecoration(labelText: 'Modele'),
              ),
            ),
          ],
        ),
        const SizedBox(height: 12),
        Row(
          children: [
            Expanded(
              child: TextField(
                controller: _vehicleColorController,
                textCapitalization: TextCapitalization.words,
                decoration: const InputDecoration(labelText: 'Couleur'),
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: TextField(
                controller: _vehicleOwnerController,
                textCapitalization: TextCapitalization.words,
                decoration: const InputDecoration(labelText: 'Proprietaire'),
              ),
            ),
          ],
        ),
      ],
    );
  }

  Widget _buildLocationStep() {
    return Column(
      children: [
        SectionPanel(
          child: Column(
            children: [
              TextField(
                controller: _locationController,
                decoration: const InputDecoration(
                  labelText: 'Lieu',
                  prefixIcon: Icon(Icons.place_outlined),
                ),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: _notesController,
                maxLines: 3,
                decoration: const InputDecoration(
                  labelText: 'Notes internes',
                  alignLabelWithHint: true,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        SectionPanel(
          child: Row(
            children: [
              Expanded(
                child: Text(
                  _position == null
                      ? 'GPS non capture'
                      : 'GPS: ${_position!.latitude.toStringAsFixed(6)}, ${_position!.longitude.toStringAsFixed(6)}',
                  style: const TextStyle(color: apmMuted),
                ),
              ),
              TextButton.icon(
                onPressed: _locating ? null : _captureGps,
                icon: _locating
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.my_location),
                label: const Text('GPS'),
              ),
            ],
          ),
        ),
      ],
    );
  }

  Widget _buildPhotosStep() {
    return SectionPanel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Expanded(
                child: Text(
                  'Photos preuve',
                  style: TextStyle(fontWeight: FontWeight.w900),
                ),
              ),
              Text(
                '${_photos.length}',
                style: const TextStyle(color: apmMuted),
              ),
            ],
          ),
          const SizedBox(height: 12),
          if (_photos.isNotEmpty)
            GridView.count(
              crossAxisCount: 3,
              shrinkWrap: true,
              physics: const NeverScrollableScrollPhysics(),
              mainAxisSpacing: 8,
              crossAxisSpacing: 8,
              children: [
                for (final photo in _photos)
                  _LocalPhotoTile(
                    photo: photo,
                    onDelete: () => _removePhoto(photo),
                  ),
              ],
            )
          else
            const Text('Aucune photo', style: TextStyle(color: apmMuted)),
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: OutlinedButton.icon(
                  onPressed: () => _pickPhoto(ImageSource.camera),
                  icon: const Icon(Icons.photo_camera_outlined),
                  label: const Text('Camera'),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: OutlinedButton.icon(
                  onPressed: () => _pickPhoto(ImageSource.gallery),
                  icon: const Icon(Icons.photo_library_outlined),
                  label: const Text('Galerie'),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildReviewStep() {
    final selected = _selectedInterventions;
    return SectionPanel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _ReviewRow(
            label: 'Type',
            value: _subjectLabel(
              _vehicleInvolved
                  ? PvSubjectTypes.personWithVehicle
                  : PvSubjectTypes.personOnly,
            ),
          ),
          _ReviewInfractionsRow(selected: selected),
          _ReviewRow(label: 'Montant indicatif', value: formatFcfa(_totalFcfa)),
          _ReviewRow(
            label: 'Type de personne',
            value: _subjectKind == 'MORALE'
                ? 'Personne morale'
                : 'Personne physique',
          ),
          _ReviewRow(
            label: 'Contrevenant',
            value: _subjectKind == 'MORALE'
                ? _summaryText([
                    _clean(_raisonSocialeController.text),
                    if (_clean(_identityNumberController.text) != null)
                      '${_identityType ?? '-'} ${_clean(_identityNumberController.text)}',
                  ])
                : _summaryText([
                    _clean(_lastNameController.text),
                    _clean(_firstNameController.text),
                    if (_clean(_identityNumberController.text) != null)
                      '${_identityType ?? '-'} ${_clean(_identityNumberController.text)}',
                  ]),
          ),
          _ReviewRow(
            label: 'Vehicule',
            value: _summaryText([
              _clean(_plateController.text),
              if (_clean(_registrationCardController.text) != null)
                'CG ${_clean(_registrationCardController.text)}',
              _clean(_vehicleMakeController.text),
              _clean(_vehicleModelController.text),
              _clean(_vehicleColorController.text),
            ]),
          ),
          _ReviewRow(
            label: 'Lieu',
            value: _clean(_locationController.text) ?? '-',
          ),
          _ReviewRow(label: 'Photos', value: '${_photos.length}'),
          if (!_editing) ...[
            const Divider(height: 24),
            const Text(
              'Sans reseau, cette saisie sera conservee comme brouillon local. Elle deviendra un PV officiel apres synchronisation serveur.',
              style: TextStyle(color: apmMuted),
            ),
          ],
        ],
      ),
    );
  }

  String _subjectLabel(String value) => switch (value) {
    PvSubjectTypes.personOnly => 'Usager sans vehicule',
    PvSubjectTypes.vehicleOnly => 'Vehicule sans conducteur',
    _ => 'Usager avec vehicule',
  };
}

class _ReviewInfractionsRow extends StatelessWidget {
  const _ReviewInfractionsRow({required this.selected});

  final List<Intervention> selected;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Flexible(
            flex: 4,
            child: Text(
              'Infractions',
              style: TextStyle(color: apmMuted, fontWeight: FontWeight.w700),
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            flex: 6,
            child: selected.isEmpty
                ? const Text('-')
                : Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      for (final item in selected)
                        Padding(
                          padding: const EdgeInsets.only(bottom: 4),
                          child: Text(item.nom),
                        ),
                    ],
                  ),
          ),
        ],
      ),
    );
  }
}

class _LocalPhotoTile extends StatelessWidget {
  const _LocalPhotoTile({required this.photo, required this.onDelete});

  final PvDraftPhoto photo;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(8),
      child: Stack(
        fit: StackFit.expand,
        children: [
          Image.file(
            File(photo.path),
            fit: BoxFit.cover,
            errorBuilder: (_, _, _) => Container(
              color: apmPanel,
              child: const Icon(Icons.broken_image_outlined, color: apmMuted),
            ),
          ),
          Positioned(
            top: 2,
            right: 2,
            child: Material(
              color: Colors.black54,
              shape: const CircleBorder(),
              child: IconButton(
                iconSize: 16,
                padding: const EdgeInsets.all(4),
                constraints: const BoxConstraints(),
                icon: const Icon(Icons.close, color: Colors.white),
                onPressed: onDelete,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _ReviewRow extends StatelessWidget {
  const _ReviewRow({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Flexible(
            flex: 4,
            child: Text(
              label,
              softWrap: true,
              style: const TextStyle(
                color: apmMuted,
                fontWeight: FontWeight.w700,
              ),
            ),
          ),
          const SizedBox(width: 12),
          Expanded(flex: 6, child: Text(value.isEmpty ? '-' : value)),
        ],
      ),
    );
  }
}
