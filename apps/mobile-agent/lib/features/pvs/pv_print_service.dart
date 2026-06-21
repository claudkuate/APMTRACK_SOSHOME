import 'package:thermal_printer_plus/thermal_printer.dart';

import '../../core/models.dart';
import 'pv_ticket_builder.dart';

class PvPrintException implements Exception {
  const PvPrintException(this.message);

  final String message;

  @override
  String toString() => message;
}

class PvPrintService {
  const PvPrintService({this.ticketBuilder = const PvTicketBuilder()});

  final PvTicketBuilder ticketBuilder;

  Stream<PrinterDevice> scanBluetoothPrinters() {
    return PrinterManager.instance.discovery(
      type: PrinterType.bluetooth,
      isBle: false,
    );
  }

  Future<void> printPv({
    required Pv pv,
    required PrinterDevice printer,
    MobileProfile? profile,
  }) async {
    final address = printer.address;
    if (address == null || address.trim().isEmpty) {
      throw const PvPrintException('Adresse Bluetooth imprimante introuvable');
    }

    final input = BluetoothPrinterInput(address: address, name: printer.name);
    final connected = await PrinterManager.instance.connect(
      type: PrinterType.bluetooth,
      model: input,
    );
    if (!connected) {
      throw const PvPrintException('Connexion imprimante impossible');
    }

    final sent = await PrinterManager.instance.send(
      type: PrinterType.bluetooth,
      bytes: await buildPvTicketBytes(pv: pv, profile: profile),
    );
    await PrinterManager.instance.disconnect(
      type: PrinterType.bluetooth,
      delayMs: 300,
    );
    if (!sent) {
      throw const PvPrintException('Impression refusee par imprimante');
    }
  }

  Future<List<int>> buildPvTicketBytes({
    required Pv pv,
    MobileProfile? profile,
    String? verificationBaseUrl,
  }) {
    return ticketBuilder.buildPvTicketBytes(
      pv: pv,
      profile: profile,
      verificationBaseUrl: verificationBaseUrl,
    );
  }

  String buildVerificationUrl(Pv pv, {String? baseUrl}) {
    return ticketBuilder.buildVerificationUrl(pv, baseUrl: baseUrl);
  }
}
