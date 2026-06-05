import 'package:flutter/material.dart';

const apmGreen = Color(0xFF1F7A4D);
const apmRed = Color(0xFFB42318);
const apmGold = Color(0xFFE0A106);
const apmCanvas = Color(0xFFF6F7F9);
const apmPanel = Color(0xFFFFFFFF);
const apmBorder = Color(0xFFD7DBE0);
const apmText = Color(0xFF17202A);
const apmMuted = Color(0xFF5F6B7A);

ThemeData buildApmtrackTheme() {
  final scheme = ColorScheme.fromSeed(
    seedColor: apmGreen,
    primary: apmGreen,
    error: apmRed,
    surface: apmPanel,
  );

  return ThemeData(
    colorScheme: scheme,
    scaffoldBackgroundColor: apmCanvas,
    useMaterial3: true,
    fontFamily: 'Arial',
    appBarTheme: const AppBarTheme(
      centerTitle: false,
      backgroundColor: apmPanel,
      foregroundColor: apmText,
      elevation: 0,
      surfaceTintColor: apmPanel,
    ),
    cardTheme: CardThemeData(
      color: apmPanel,
      elevation: 0,
      margin: EdgeInsets.zero,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(8),
        side: const BorderSide(color: apmBorder),
      ),
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        minimumSize: const Size.fromHeight(48),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
      ),
    ),
    inputDecorationTheme: InputDecorationTheme(
      border: OutlineInputBorder(borderRadius: BorderRadius.circular(8)),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(8),
        borderSide: const BorderSide(color: apmBorder),
      ),
      filled: true,
      fillColor: apmPanel,
    ),
  );
}
