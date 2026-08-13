import 'package:flutter/material.dart';

import '../../core/auth/session_controller.dart';
import '../../core/theme.dart';

/// Sortie obligatoire du provisionnement automatique.
///
/// Un agent cree depuis le back-office recoit un mot de passe temporaire connu de son
/// administrateur : tant qu'il ne l'a pas remplace, aucun autre ecran n'est accessible.
class ChangePasswordPage extends StatefulWidget {
  const ChangePasswordPage({super.key, required this.controller});

  final SessionController controller;

  @override
  State<ChangePasswordPage> createState() => _ChangePasswordPageState();
}

class _ChangePasswordPageState extends State<ChangePasswordPage> {
  final _formKey = GlobalKey<FormState>();
  final _currentController = TextEditingController();
  final _passwordController = TextEditingController();
  final _confirmController = TextEditingController();
  bool _submitting = false;
  String? _error;

  @override
  void dispose() {
    _currentController.dispose();
    _passwordController.dispose();
    _confirmController.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (!(_formKey.currentState?.validate() ?? false)) {
      return;
    }
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      await widget.controller.changePassword(
        _currentController.text,
        _passwordController.text,
      );
    } catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _submitting = false;
        _error = error.toString();
      });
      return;
    }
    if (!mounted) {
      return;
    }
    setState(() => _submitting = false);
  }

  @override
  Widget build(BuildContext context) {
    final fullName = widget.controller.session?.user.fullName ?? '';
    return Scaffold(
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 420),
              child: Form(
                key: _formKey,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Text(
                      'Definissez votre mot de passe',
                      style: Theme.of(context).textTheme.titleLarge?.copyWith(
                        color: apmText,
                        fontWeight: FontWeight.w900,
                      ),
                    ),
                    const SizedBox(height: 8),
                    Text(
                      fullName.isEmpty
                          ? 'Votre compte utilise encore un mot de passe temporaire.'
                          : '$fullName, votre compte utilise encore un mot de passe temporaire.',
                      style: Theme.of(context).textTheme.bodyMedium,
                    ),
                    const SizedBox(height: 20),
                    TextFormField(
                      controller: _currentController,
                      obscureText: true,
                      decoration: const InputDecoration(
                        labelText: 'Mot de passe temporaire',
                        prefixIcon: Icon(Icons.lock_clock_outlined),
                      ),
                      validator: (value) => (value ?? '').isEmpty
                          ? 'Mot de passe temporaire requis'
                          : null,
                    ),
                    const SizedBox(height: 12),
                    TextFormField(
                      controller: _passwordController,
                      obscureText: true,
                      decoration: const InputDecoration(
                        labelText: 'Nouveau mot de passe',
                        prefixIcon: Icon(Icons.lock_outline),
                      ),
                      validator: (value) {
                        final password = value ?? '';
                        if (password.length < 8) {
                          // Meme minimum que le serveur (`auth::hash_password`).
                          return 'Au moins 8 caracteres';
                        }
                        if (password == _currentController.text) {
                          return 'Choisissez un mot de passe different';
                        }
                        return null;
                      },
                    ),
                    const SizedBox(height: 12),
                    TextFormField(
                      controller: _confirmController,
                      obscureText: true,
                      textInputAction: TextInputAction.done,
                      onFieldSubmitted: (_) => _submit(),
                      decoration: const InputDecoration(
                        labelText: 'Confirmer le mot de passe',
                        prefixIcon: Icon(Icons.lock_reset_outlined),
                      ),
                      validator: (value) => value == _passwordController.text
                          ? null
                          : 'Les mots de passe ne correspondent pas',
                    ),
                    if (_error != null) ...[
                      const SizedBox(height: 12),
                      Text(
                        _error!,
                        style: TextStyle(
                          color: Theme.of(context).colorScheme.error,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ],
                    const SizedBox(height: 20),
                    FilledButton.icon(
                      onPressed: _submitting ? null : _submit,
                      icon: _submitting
                          ? const SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Icon(Icons.check),
                      label: const Text('Enregistrer'),
                    ),
                    const SizedBox(height: 8),
                    TextButton(
                      onPressed: _submitting
                          ? null
                          : () => widget.controller.signOut(),
                      child: const Text('Se deconnecter'),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
