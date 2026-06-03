import 'package:flutter_test/flutter_test.dart';

import 'package:apmtrack_agent/main.dart';

void main() {
  testWidgets('renders agent shell', (WidgetTester tester) async {
    await tester.pumpWidget(const ApmtrackAgentApp());

    expect(find.text('APMTRACK Agent'), findsOneWidget);
    expect(find.text('Statut plateforme'), findsOneWidget);
    expect(find.text('Verifier API'), findsOneWidget);
  });
}

