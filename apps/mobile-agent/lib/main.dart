import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;

const apiBaseUrl = String.fromEnvironment(
  'API_URL',
  defaultValue: 'http://10.0.2.2:8080',
);
const appEnvironment = String.fromEnvironment(
  'APP_ENV',
  defaultValue: 'development',
);

void main() {
  runApp(const ApmtrackAgentApp());
}

class ApmtrackAgentApp extends StatelessWidget {
  const ApmtrackAgentApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'APMTRACK Agent',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF1F7A4D)),
        scaffoldBackgroundColor: const Color(0xFFF6F7F9),
        useMaterial3: true,
      ),
      home: const AgentHomePage(),
    );
  }
}

class AgentHomePage extends StatefulWidget {
  const AgentHomePage({super.key});

  @override
  State<AgentHomePage> createState() => _AgentHomePageState();
}

class _AgentHomePageState extends State<AgentHomePage> {
  String _status = 'non verifie';
  String _detail = apiBaseUrl;
  bool _loading = false;

  Future<void> _checkApi() async {
    setState(() {
      _loading = true;
      _status = 'verification';
      _detail = apiBaseUrl;
    });

    try {
      final response = await http
          .get(Uri.parse('$apiBaseUrl/health'))
          .timeout(const Duration(seconds: 5));

      if (!mounted) return;

      if (response.statusCode == 200) {
        final payload = jsonDecode(response.body) as Map<String, dynamic>;
        setState(() {
          _status = payload['status']?.toString() ?? 'ok';
          _detail =
              '${payload['service'] ?? 'apmtrack-api'} ${payload['version'] ?? ''}';
        });
      } else {
        setState(() {
          _status = 'erreur';
          _detail = 'HTTP ${response.statusCode}';
        });
      }
    } on TimeoutException {
      if (!mounted) return;
      setState(() {
        _status = 'indisponible';
        _detail = 'Delai depasse';
      });
    } catch (_) {
      if (!mounted) return;
      setState(() {
        _status = 'indisponible';
        _detail = 'Endpoint /health inaccessible';
      });
    } finally {
      if (mounted) {
        setState(() {
          _loading = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;

    return Scaffold(
      appBar: AppBar(
        title: const Text('APMTRACK Agent'),
        backgroundColor: colorScheme.surface,
      ),
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _HeaderCard(status: _status, detail: _detail),
              const SizedBox(height: 16),
              _InfoRow(label: 'Environnement', value: appEnvironment),
              _InfoRow(label: 'API', value: apiBaseUrl),
              const Spacer(),
              FilledButton(
                onPressed: _loading ? null : _checkApi,
                child: Text(_loading ? 'Verification...' : 'Verifier API'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _HeaderCard extends StatelessWidget {
  const _HeaderCard({required this.status, required this.detail});

  final String status;
  final String detail;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(18),
      decoration: BoxDecoration(
        color: Colors.white,
        border: Border.all(color: const Color(0xFFD7DBE0)),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'Statut plateforme',
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 12),
          Text(
            status,
            style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                  color: const Color(0xFF1F7A4D),
                  fontWeight: FontWeight.w700,
                ),
          ),
          const SizedBox(height: 8),
          Text(
            detail,
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: const Color(0xFF5F6B7A),
                ),
          ),
        ],
      ),
    );
  }
}

class _InfoRow extends StatelessWidget {
  const _InfoRow({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 132,
            child: Text(
              label,
              style: const TextStyle(
                color: Color(0xFF5F6B7A),
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
          Expanded(
            child: Text(
              value,
              softWrap: true,
            ),
          ),
        ],
      ),
    );
  }
}

