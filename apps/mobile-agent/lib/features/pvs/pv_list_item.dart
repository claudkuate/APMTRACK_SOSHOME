import 'package:flutter/material.dart';

import '../../core/auth/session_controller.dart';
import '../../core/models.dart';
import '../../core/theme.dart';
import '../../core/ui/common.dart';
import 'pv_detail_page.dart';

class PvListItems extends StatelessWidget {
  const PvListItems({
    super.key,
    required this.controller,
    required this.pvs,
    this.includeAmountAndDate = false,
    this.titleWeight = FontWeight.w800,
  });

  final SessionController controller;
  final List<Pv> pvs;
  final bool includeAmountAndDate;
  final FontWeight titleWeight;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        for (var index = 0; index < pvs.length; index += 1) ...[
          PvListItem(
            controller: controller,
            pv: pvs[index],
            includeAmountAndDate: includeAmountAndDate,
            titleWeight: titleWeight,
          ),
          if (index < pvs.length - 1)
            const Divider(height: 1, indent: 16, endIndent: 16),
        ],
      ],
    );
  }
}

class PvListItem extends StatelessWidget {
  const PvListItem({
    super.key,
    required this.controller,
    required this.pv,
    this.includeAmountAndDate = false,
    this.titleWeight = FontWeight.w800,
  });

  final SessionController controller;
  final Pv pv;
  final bool includeAmountAndDate;
  final FontWeight titleWeight;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      onTap: () => Navigator.of(context).push(
        MaterialPageRoute(
          builder: (_) => PvDetailPage(controller: controller, pv: pv),
        ),
      ),
      title: Text(pv.pvNumber, style: TextStyle(fontWeight: titleWeight)),
      subtitle: Text(
        [
          pv.subjectLabel,
          pv.infractionsLabel,
          pv.vehicleIdentityLabel,
          pv.verbalizedDisplayName,
          if (includeAmountAndDate) formatFcfa(pv.amountInitialFcfa),
          if (includeAmountAndDate) formatShortDate(pv.createdAt),
        ].whereType<String>().join(' - '),
      ),
      trailing: StatusPill(status: pv.status),
      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
      shape: const RoundedRectangleBorder(
        side: BorderSide(color: Colors.transparent),
      ),
      hoverColor: apmGreen.withValues(alpha: 0.04),
    );
  }
}
