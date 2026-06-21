import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:thermal_printer_plus/thermal_printer.dart';

import '../../core/api/api_client.dart';
import '../../core/auth/session_controller.dart';
import '../../core/models.dart';
import '../../core/theme.dart';
import '../../core/ui/common.dart';
import 'create_pv_page.dart';
import 'pv_photos_section.dart';
import 'pv_print_service.dart';

class PvDetailPage extends StatefulWidget {
  const PvDetailPage({
    super.key,
    required this.controller,
    required this.pv,
    this.printService = const PvPrintService(),
  });

  final SessionController controller;
  final Pv pv;
  final PvPrintService printService;

  @override
  State<PvDetailPage> createState() => _PvDetailPageState();
}

class _PvDetailPageState extends State<PvDetailPage> {
  late final Future<String> _qrFuture = widget.controller.pvQrSvg(widget.pv.id);

  @override
  Widget build(BuildContext context) {
    final pv = widget.pv;
    return Scaffold(
      appBar: AppBar(
        title: Text(pv.pvNumber),
        actions: [
          IconButton(
            tooltip: 'Imprimer',
            icon: const Icon(Icons.print_outlined),
            onPressed: () => _openPrintSheet(context),
          ),
          if (pv.canEdit)
            IconButton(
              tooltip: 'Modifier',
              icon: const Icon(Icons.edit_outlined),
              onPressed: () => Navigator.of(context).pushReplacement(
                MaterialPageRoute(
                  builder: (_) => CreatePvPage(
                    controller: widget.controller,
                    initialPv: pv,
                  ),
                ),
              ),
            ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          SectionPanel(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        pv.pvNumber,
                        style: Theme.of(context).textTheme.titleLarge?.copyWith(
                          fontWeight: FontWeight.w900,
                        ),
                      ),
                    ),
                    StatusPill(status: pv.status),
                  ],
                ),
                const SizedBox(height: 16),
                _DetailRow(
                  label: 'Montant',
                  value: formatFcfa(pv.amountInitialFcfa),
                ),
                _DetailRow(label: 'Date', value: formatShortDate(pv.createdAt)),
                _DetailRow(label: 'Type', value: pv.subjectLabel),
                _DetailRow(label: 'Plaque', value: pv.vehiclePlate ?? '-'),
                _DetailRow(
                  label: 'Carte grise',
                  value: pv.vehicleRegistrationCardNumber ?? '-',
                ),
                _DetailRow(label: 'Marque', value: pv.vehicleMake ?? '-'),
                _DetailRow(label: 'Modele', value: pv.vehicleModel ?? '-'),
                _DetailRow(label: 'Couleur', value: pv.vehicleColor ?? '-'),
                _DetailRow(
                  label: 'Proprietaire',
                  value: pv.vehicleOwnerName ?? '-',
                ),
                _DetailRow(
                  label: 'Verbalise',
                  value: pv.verbalizedDisplayName ?? '-',
                ),
                _DetailRow(
                  label: 'Piece',
                  value: pv.verbalizedIdentityLabel ?? '-',
                ),
                _DetailRow(
                  label: 'Telephone',
                  value: pv.verbalizedPhone ?? '-',
                ),
                _DetailRow(
                  label: 'Adresse',
                  value: pv.verbalizedAddress ?? '-',
                ),
                _DetailRow(label: 'Lieu', value: pv.locationDescription ?? '-'),
                _DetailRow(
                  label: 'GPS',
                  value: pv.gpsLatitude == null || pv.gpsLongitude == null
                      ? '-'
                      : '${pv.gpsLatitude}, ${pv.gpsLongitude}',
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
          SectionPanel(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text(
                  'Infractions',
                  style: TextStyle(fontWeight: FontWeight.w900),
                ),
                const SizedBox(height: 8),
                if (pv.interventions.isEmpty)
                  const Text('-', style: TextStyle(color: apmMuted))
                else
                  for (final item in pv.interventions)
                    Padding(
                      padding: const EdgeInsets.symmetric(vertical: 5),
                      child: Row(
                        children: [
                          Expanded(
                            child: Text(
                              item.nom,
                              style: const TextStyle(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                          ),
                          Text(
                            formatFcfa(item.montantFcfa),
                            style: const TextStyle(color: apmMuted),
                          ),
                        ],
                      ),
                    ),
              ],
            ),
          ),
          const SizedBox(height: 12),
          SectionPanel(
            child: Column(
              children: [
                const Text(
                  'QR officiel de verification',
                  style: TextStyle(fontWeight: FontWeight.w900),
                ),
                const SizedBox(height: 12),
                FutureBuilder<String>(
                  future: _qrFuture,
                  builder: (context, snapshot) {
                    if (snapshot.connectionState != ConnectionState.done) {
                      return const SizedBox(
                        height: 180,
                        child: Center(child: CircularProgressIndicator()),
                      );
                    }
                    if (snapshot.hasError || snapshot.data == null) {
                      final message = snapshot.error is ApiException
                          ? (snapshot.error as ApiException).message
                          : 'QR indisponible';
                      return Text(
                        message,
                        textAlign: TextAlign.center,
                        style: const TextStyle(color: apmRed),
                      );
                    }
                    return SvgPicture.string(
                      snapshot.data!,
                      height: 210,
                      width: 210,
                    );
                  },
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
          PvPhotosSection(
            controller: widget.controller,
            pvId: pv.id,
            editable: pv.canEdit,
          ),
        ],
      ),
    );
  }

  void _openPrintSheet(BuildContext context) {
    showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (_) => _PvPrintSheet(
        pv: widget.pv,
        profile: widget.controller.profile,
        printService: widget.printService,
      ),
    );
  }
}

class _PvPrintSheet extends StatefulWidget {
  const _PvPrintSheet({
    required this.pv,
    required this.profile,
    required this.printService,
  });

  final Pv pv;
  final MobileProfile? profile;
  final PvPrintService printService;

  @override
  State<_PvPrintSheet> createState() => _PvPrintSheetState();
}

class _PvPrintSheetState extends State<_PvPrintSheet> {
  final List<PrinterDevice> _printers = [];
  StreamSubscription<PrinterDevice>? _scanSubscription;
  bool _scanning = false;
  bool _printing = false;
  String? _error;
  String? _message;

  @override
  void initState() {
    super.initState();
    _scan();
  }

  @override
  void dispose() {
    _scanSubscription?.cancel();
    super.dispose();
  }

  Future<void> _scan() async {
    await _scanSubscription?.cancel();
    setState(() {
      _printers.clear();
      _scanning = true;
      _error = null;
      _message = null;
    });
    _scanSubscription = widget.printService.scanBluetoothPrinters().listen(
      (printer) {
        if (!mounted) return;
        setState(() {
          if (!_printers.any((item) => item.address == printer.address)) {
            _printers.add(printer);
          }
        });
      },
      onError: (Object error) {
        if (!mounted) return;
        setState(() {
          _error = 'Recherche imprimante impossible : $error';
          _scanning = false;
        });
      },
      onDone: () {
        if (!mounted) return;
        setState(() => _scanning = false);
      },
    );
  }

  Future<void> _print(PrinterDevice printer) async {
    setState(() {
      _printing = true;
      _error = null;
      _message = null;
    });
    try {
      await widget.printService.printPv(
        pv: widget.pv,
        printer: printer,
        profile: widget.profile,
      );
      if (!mounted) return;
      setState(() => _message = 'PV envoye a l imprimante');
    } on PvPrintException catch (error) {
      if (!mounted) return;
      setState(() => _error = error.message);
    } catch (error) {
      if (!mounted) return;
      setState(() => _error = 'Impression impossible : $error');
    } finally {
      if (mounted) {
        setState(() => _printing = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                const Expanded(
                  child: Text(
                    'Imprimer le PV',
                    style: TextStyle(fontWeight: FontWeight.w900, fontSize: 18),
                  ),
                ),
                IconButton(
                  tooltip: 'Rechercher',
                  onPressed: _scanning || _printing ? null : _scan,
                  icon: const Icon(Icons.refresh),
                ),
              ],
            ),
            if (_scanning) const LinearProgressIndicator(),
            if (_scanning) const SizedBox(height: 12),
            if (_printers.isEmpty)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 16),
                child: Text(
                  _scanning
                      ? 'Recherche des imprimantes Bluetooth...'
                      : 'Aucune imprimante Bluetooth trouvee.',
                  style: const TextStyle(color: apmMuted),
                ),
              )
            else
              ConstrainedBox(
                constraints: const BoxConstraints(maxHeight: 320),
                child: ListView.separated(
                  shrinkWrap: true,
                  itemCount: _printers.length,
                  separatorBuilder: (_, _) => const Divider(height: 1),
                  itemBuilder: (context, index) {
                    final printer = _printers[index];
                    return ListTile(
                      enabled: !_printing,
                      leading: const Icon(Icons.print_outlined),
                      title: Text(printer.name),
                      subtitle: Text(printer.address ?? '-'),
                      trailing: _printing
                          ? const SizedBox(
                              width: 18,
                              height: 18,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Icon(Icons.chevron_right),
                      onTap: _printing ? null : () => _print(printer),
                    );
                  },
                ),
              ),
            if (_error != null) ...[
              const SizedBox(height: 12),
              Text(
                _error!,
                style: const TextStyle(
                  color: apmRed,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ],
            if (_message != null) ...[
              const SizedBox(height: 12),
              Text(
                _message!,
                style: const TextStyle(
                  color: apmGreen,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _DetailRow extends StatelessWidget {
  const _DetailRow({required this.label, required this.value});

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
            width: 100,
            child: Text(
              label,
              style: const TextStyle(
                color: apmMuted,
                fontWeight: FontWeight.w700,
              ),
            ),
          ),
          Expanded(child: Text(value)),
        ],
      ),
    );
  }
}
