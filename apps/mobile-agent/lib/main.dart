import 'package:flutter/material.dart';
import 'package:intl/date_symbol_data_local.dart';

import 'app.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // Charge les données de locale utilisées par `DateFormat`/`NumberFormat`
  // (voir `core/ui/common.dart`, locale `fr_FR`) avant de construire l'UI.
  await initializeDateFormatting('fr_FR');
  runApp(const ApmtrackAgentApp());
}
