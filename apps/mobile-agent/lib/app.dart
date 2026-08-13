import 'package:flutter/material.dart';

import 'core/api/api_client.dart';
import 'core/auth/session_controller.dart';
import 'core/auth/session_store.dart';
import 'core/offline/offline_store.dart';
import 'core/theme.dart';
import 'features/auth/change_password_page.dart';
import 'features/auth/login_page.dart';
import 'features/home/agent_shell.dart';

class ApmtrackAgentApp extends StatefulWidget {
  const ApmtrackAgentApp({super.key, this.api, this.store, this.cache});

  final ApmtrackApi? api;
  final SessionStore? store;
  final OfflineCacheStore? cache;

  @override
  State<ApmtrackAgentApp> createState() => _ApmtrackAgentAppState();
}

class _ApmtrackAgentAppState extends State<ApmtrackAgentApp> {
  late final SessionController controller;

  @override
  void initState() {
    super.initState();
    controller = SessionController(
      api: widget.api ?? HttpApmtrackApi(),
      store: widget.store ?? SecureSessionStore(),
      cache: widget.cache,
    );
    controller.bootstrap();
  }

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'G-APM Agent',
      debugShowCheckedModeBanner: false,
      theme: buildApmtrackTheme(),
      home: AnimatedBuilder(
        animation: controller,
        builder: (context, _) {
          return switch (controller.status) {
            SessionStatus.booting => const _BootScreen(),
            SessionStatus.unauthenticated => LoginPage(controller: controller),
            SessionStatus.mustChangePassword => ChangePasswordPage(
              controller: controller,
            ),
            SessionStatus.authenticated => AgentShell(controller: controller),
          };
        },
      ),
    );
  }
}

class _BootScreen extends StatelessWidget {
  const _BootScreen();

  @override
  Widget build(BuildContext context) {
    return const Scaffold(body: Center(child: CircularProgressIndicator()));
  }
}
