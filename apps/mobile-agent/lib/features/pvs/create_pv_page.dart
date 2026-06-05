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
    'Type',
    'Infractions',
    'Sujet',
    'Localisation',
    'Preuves',
    'Revue',
  ];

  final _picker = ImagePicker();
  final _firstNameController = TextEditingController();
  final _lastNameController = TextEditingController();
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

  int _step = 0;
  String _subjectType = PvSubjectTypes.personWithVehicle;
  String? _identityType;
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
    _subjectType = pv.subjectType;
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
    super.dispose();
  }

  List<Intervention> get _selectedInterventions {
    return widget.controller.interventions
        .where((item) => _selectedInterventionIds.contains(item.id))
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
        await _uploadLocalPhotos(pv.id);
        if (!mounted) return;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('PV mis a jour: ${pv.pvNumber}')),
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
            content: Text('Hors-ligne : PV et preuves enregistres localement.'),
          ),
        );
        Navigator.of(context).pop(true);
        return;
      }
      final pv = outcome.pv!;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('PV valide serveur: ${pv.pvNumber}')),
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
            : 'Creation PV impossible',
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
    final hasPerson = _subjectType != PvSubjectTypes.vehicleOnly;
    final hasVehicle = _subjectType != PvSubjectTypes.personOnly;
    final identityNumber = hasPerson
        ? _clean(_identityNumberController.text)
        : null;
    final firstName = hasPerson ? _clean(_firstNameController.text) : null;
    final lastName = hasPerson ? _clean(_lastNameController.text) : null;
    final name = [firstName, lastName].whereType<String>().join(' ');
    return CreatePvPayload(
      interventionId: ids.first,
      interventionIds: ids,
      subjectType: _subjectType,
      verbalizedName: name.isEmpty ? null : name,
      verbalizedIdentifier: identityNumber,
      verbalizedFirstName: firstName,
      verbalizedLastName: lastName,
      verbalizedIdentityType: identityNumber == null ? null : _identityType,
      verbalizedIdentityNumber: identityNumber,
      verbalizedPhone: hasPerson ? _clean(_phoneController.text) : null,
      verbalizedAddress: hasPerson ? _clean(_addressController.text) : null,
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
      return 1;
    }
    final hasPerson =
        _clean(_firstNameController.text) != null ||
        _clean(_lastNameController.text) != null ||
        _clean(_identityNumberController.text) != null;
    final hasIdentityNumber = _clean(_identityNumberController.text) != null;
    final hasIdentityType = _identityType != null && _identityType!.isNotEmpty;
    final hasVehicle =
        _clean(_plateController.text) != null ||
        _clean(_registrationCardController.text) != null;
    if (_subjectType != PvSubjectTypes.vehicleOnly &&
        hasIdentityNumber &&
        !hasIdentityType) {
      _setError('Type d identite requis avec le numero');
      return 2;
    }
    if (_subjectType == PvSubjectTypes.personOnly && !hasPerson) {
      _setError('Nom, prenom ou numero d identite requis');
      return 2;
    }
    if (_subjectType == PvSubjectTypes.vehicleOnly && !hasVehicle) {
      _setError('Plaque ou carte grise requise');
      return 2;
    }
    if (_subjectType == PvSubjectTypes.personWithVehicle &&
        (!hasPerson || !hasVehicle)) {
      _setError('Contrevenant et plaque ou carte grise requis');
      return 2;
    }
    if (_clean(_locationController.text) == null) {
      _setError('Lieu requis');
      return 3;
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
    final title = _editing ? 'Modifier PV' : 'Nouveau PV';
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
                        ? 'Validation...'
                        : _step == _stepLabels.length - 1
                        ? (_editing ? 'Mettre a jour' : 'Valider')
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
      0 => _buildTypeStep(),
      1 => _buildInfractionsStep(),
      2 => _buildSubjectStep(),
      3 => _buildLocationStep(),
      4 => _buildPhotosStep(),
      _ => _buildReviewStep(),
    };
  }

  Widget _buildTypeStep() {
    return SectionPanel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SegmentedButton<String>(
            segments: const [
              ButtonSegment(
                value: PvSubjectTypes.personWithVehicle,
                icon: Icon(Icons.person_pin_circle_outlined),
                label: Text('Avec vehicule'),
              ),
              ButtonSegment(
                value: PvSubjectTypes.personOnly,
                icon: Icon(Icons.person_outline),
                label: Text('Sans vehicule'),
              ),
              ButtonSegment(
                value: PvSubjectTypes.vehicleOnly,
                icon: Icon(Icons.directions_car_outlined),
                label: Text('Vehicule seul'),
              ),
            ],
            selected: {_subjectType},
            onSelectionChanged: (values) {
              setState(() => _subjectType = values.single);
            },
          ),
        ],
      ),
    );
  }

  Widget _buildInfractionsStep() {
    final interventions = widget.controller.interventions;
    return SectionPanel(
      padding: EdgeInsets.zero,
      child: Column(
        children: [
          if (interventions.isEmpty)
            const Padding(
              padding: EdgeInsets.all(16),
              child: EmptyState(
                title: 'Aucune infraction',
                message: 'Le referentiel mobile est vide.',
                icon: Icons.rule_folder_outlined,
              ),
            )
          else
            for (final item in interventions)
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
                subtitle: Text(formatFcfa(item.montantFcfa)),
              ),
          const Divider(height: 1),
          ListTile(
            title: const Text('Total'),
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
          if (_subjectType != PvSubjectTypes.vehicleOnly) _buildPersonFields(),
          if (_subjectType == PvSubjectTypes.personWithVehicle)
            const SizedBox(height: 16),
          if (_subjectType != PvSubjectTypes.personOnly) _buildVehicleFields(),
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
        const Text('Vehicule', style: TextStyle(fontWeight: FontWeight.w900)),
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
          _ReviewRow(label: 'Type', value: _subjectLabel(_subjectType)),
          _ReviewRow(
            label: 'Infractions',
            value: selected.isEmpty
                ? '-'
                : selected.map((item) => item.nom).join(' / '),
          ),
          _ReviewRow(label: 'Montant', value: formatFcfa(_totalFcfa)),
          _ReviewRow(
            label: 'Contrevenant',
            value: _summaryText([
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
          SizedBox(
            width: 96,
            child: Text(
              label,
              style: const TextStyle(
                color: apmMuted,
                fontWeight: FontWeight.w700,
              ),
            ),
          ),
          Expanded(child: Text(value.isEmpty ? '-' : value)),
        ],
      ),
    );
  }
}
