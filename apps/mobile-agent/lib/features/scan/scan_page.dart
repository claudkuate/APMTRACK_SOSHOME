import 'package:flutter/material.dart';
import 'package:mobile_scanner/mobile_scanner.dart';

import '../../core/api/api_client.dart';
import '../../core/auth/session_controller.dart';
import '../../core/models.dart';
import '../../core/theme.dart';
import '../../core/ui/common.dart';

class ScanPage extends StatefulWidget {
  const ScanPage({super.key, required this.controller});

  final SessionController controller;

  @override
  State<ScanPage> createState() => _ScanPageState();
}

class _ScanPageState extends State<ScanPage> {
  final _manualController = TextEditingController();
  PvPublic? _result;
  String? _error;
  bool _checking = false;
  bool _scanLocked = false;

  @override
  void dispose() {
    _manualController.dispose();
    super.dispose();
  }

  Future<void> _verify(String raw) async {
    final pvNumber = _extractPvNumber(raw);
    if (pvNumber.isEmpty) {
      return;
    }
    setState(() {
      _checking = true;
      _error = null;
      _result = null;
    });
    try {
      final result = await widget.controller.verifyPv(pvNumber);
      setState(() => _result = result);
    } on ApiException catch (error) {
      setState(() => _error = error.message);
    } catch (_) {
      setState(() => _error = 'Verification impossible');
    } finally {
      if (mounted) {
        setState(() => _checking = false);
      }
    }
  }

  String _extractPvNumber(String raw) {
    final value = raw.trim();
    if (value.isEmpty) return '';
    final uri = Uri.tryParse(value);
    if (uri != null && uri.pathSegments.isNotEmpty) {
      final pvsIndex = uri.pathSegments.indexOf('pvs');
      if (pvsIndex >= 0 && pvsIndex + 1 < uri.pathSegments.length) {
        return Uri.decodeComponent(uri.pathSegments[pvsIndex + 1]);
      }
    }
    return value;
  }

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 96),
      children: [
        Text(
          'Scanner un PV',
          style: Theme.of(
            context,
          ).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w900),
        ),
        const SizedBox(height: 8),
        const Text(
          'La verification publique ne revele que le statut et le montant du PV.',
          style: TextStyle(color: apmMuted),
        ),
        const SizedBox(height: 12),
        ClipRRect(
          borderRadius: BorderRadius.circular(8),
          child: SizedBox(
            height: 260,
            child: MobileScanner(
              onDetect: (capture) {
                if (_scanLocked) return;
                if (capture.barcodes.isEmpty) return;
                final value = capture.barcodes.first.rawValue;
                if (value == null || value.trim().isEmpty) return;
                _scanLocked = true;
                _verify(value).whenComplete(() async {
                  await Future<void>.delayed(const Duration(seconds: 2));
                  if (mounted) {
                    setState(() => _scanLocked = false);
                  }
                });
              },
            ),
          ),
        ),
        const SizedBox(height: 12),
        SectionPanel(
          child: Column(
            children: [
              TextField(
                controller: _manualController,
                textCapitalization: TextCapitalization.characters,
                decoration: const InputDecoration(
                  labelText: 'Numero PV ou URL QR',
                  prefixIcon: Icon(Icons.qr_code_2),
                ),
                onSubmitted: _verify,
              ),
              const SizedBox(height: 12),
              FilledButton.icon(
                onPressed: _checking
                    ? null
                    : () => _verify(_manualController.text.trim()),
                icon: _checking
                    ? const SizedBox(
                        width: 18,
                        height: 18,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.verified_outlined),
                label: const Text('Verifier'),
              ),
            ],
          ),
        ),
        if (_error != null) ...[
          const SizedBox(height: 12),
          SectionPanel(
            child: Text(
              _error!,
              style: const TextStyle(
                color: apmRed,
                fontWeight: FontWeight.w800,
              ),
            ),
          ),
        ],
        if (_result != null) ...[
          const SizedBox(height: 12),
          SectionPanel(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        _result!.pvNumber,
                        style: Theme.of(context).textTheme.titleLarge?.copyWith(
                          fontWeight: FontWeight.w900,
                        ),
                      ),
                    ),
                    StatusPill(status: _result!.status),
                  ],
                ),
                const SizedBox(height: 12),
                Text('Montant: ${formatFcfa(_result!.amountInitialFcfa)}'),
                Text('Date: ${formatShortDate(_result!.createdAt)}'),
              ],
            ),
          ),
        ],
      ],
    );
  }
}
