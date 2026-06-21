import 'package:esc_pos_utils_plus/esc_pos_utils_plus.dart';

import '../../core/models.dart';
import 'pv_ticket_content.dart';

class PvTicketBuilder {
  const PvTicketBuilder({this.contentBuilder = const PvTicketContentBuilder()});

  final PvTicketContentBuilder contentBuilder;

  Future<List<int>> buildPvTicketBytes({
    required Pv pv,
    MobileProfile? profile,
    String? verificationBaseUrl,
  }) async {
    final profileCaps = await CapabilityProfile.load();
    final generator = Generator(PaperSize.mm58, profileCaps);
    final content = contentBuilder.build(
      pv: pv,
      profile: profile,
      verificationBaseUrl: verificationBaseUrl,
    );
    final bytes = <int>[];

    void line(
      String value, {
      PosStyles styles = const PosStyles(),
      int linesAfter = 0,
    }) {
      bytes.addAll(
        generator.text(
          _ticketText(value),
          styles: styles,
          linesAfter: linesAfter,
          maxCharsPerLine: 32,
        ),
      );
    }

    bytes.addAll(generator.reset());
    line(
      content.headerLines.first,
      styles: const PosStyles(
        align: PosAlign.center,
        bold: true,
        height: PosTextSize.size2,
        width: PosTextSize.size2,
      ),
    );
    for (final header in content.headerLines.skip(1)) {
      line(header, styles: const PosStyles(align: PosAlign.center));
    }
    bytes.addAll(generator.hr(ch: '='));
    for (final detail in content.detailLines) {
      line(detail);
    }
    bytes.addAll(generator.hr());
    line('Infractions', styles: const PosStyles(bold: true));
    for (final infraction in content.infractionLines) {
      line(infraction);
    }
    bytes.addAll(generator.hr());
    line(content.totalLine);
    bytes.addAll(generator.hr());
    line(
      'Verification',
      styles: const PosStyles(align: PosAlign.center, bold: true),
    );
    bytes.addAll(
      generator.qrcode(
        content.verificationUrl,
        align: PosAlign.center,
        size: QRSize.size5,
      ),
    );
    line(
      content.verificationUrl,
      styles: const PosStyles(align: PosAlign.center),
    );
    bytes.addAll(generator.feed(2));
    bytes.addAll(generator.cut(mode: PosCutMode.partial));
    return bytes;
  }

  String buildVerificationUrl(Pv pv, {String? baseUrl}) {
    return contentBuilder.buildVerificationUrl(pv, baseUrl: baseUrl);
  }

  String _ticketText(String value) {
    final normalized = value
        .replaceAll('\u00a0', ' ')
        .replaceAll('\u202f', ' ')
        .replaceAll('\u2019', "'")
        .replaceAll('\u2018', "'")
        .replaceAll('\u201c', '"')
        .replaceAll('\u201d', '"')
        .replaceAll('\u2013', '-')
        .replaceAll('\u2014', '-');
    final buffer = StringBuffer();
    for (final rune in normalized.runes) {
      buffer.write(_printableChar(rune));
    }
    return buffer.toString();
  }

  String _printableChar(int rune) {
    return switch (rune) {
      0x00c0 || 0x00c1 || 0x00c2 || 0x00c3 || 0x00c4 || 0x00c5 => 'A',
      0x00c7 => 'C',
      0x00c8 || 0x00c9 || 0x00ca || 0x00cb => 'E',
      0x00cc || 0x00cd || 0x00ce || 0x00cf => 'I',
      0x00d1 => 'N',
      0x00d2 || 0x00d3 || 0x00d4 || 0x00d5 || 0x00d6 => 'O',
      0x00d9 || 0x00da || 0x00db || 0x00dc => 'U',
      0x00dd => 'Y',
      0x00e0 || 0x00e1 || 0x00e2 || 0x00e3 || 0x00e4 || 0x00e5 => 'a',
      0x00e7 => 'c',
      0x00e8 || 0x00e9 || 0x00ea || 0x00eb => 'e',
      0x00ec || 0x00ed || 0x00ee || 0x00ef => 'i',
      0x00f1 => 'n',
      0x00f2 || 0x00f3 || 0x00f4 || 0x00f5 || 0x00f6 => 'o',
      0x00f9 || 0x00fa || 0x00fb || 0x00fc => 'u',
      0x00fd || 0x00ff => 'y',
      _ => rune <= 0xff ? String.fromCharCode(rune) : '?',
    };
  }
}
