import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../../core/api/api_client.dart';
import '../../core/auth/session_controller.dart';
import '../../core/models.dart';
import '../../core/theme.dart';
import '../../core/ui/common.dart';
import 'create_pv_page.dart';
import 'pv_photos_section.dart';

class PvDetailPage extends StatefulWidget {
  const PvDetailPage({super.key, required this.controller, required this.pv});

  final SessionController controller;
  final Pv pv;

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
                _DetailRow(label: 'Verbalise', value: pv.verbalizedName ?? '-'),
                _DetailRow(
                  label: 'Identifiant',
                  value: pv.verbalizedIdentifier ?? '-',
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
                  'QR de verification',
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
