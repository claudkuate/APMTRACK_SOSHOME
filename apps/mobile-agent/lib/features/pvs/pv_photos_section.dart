import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart';

import '../../core/api/api_client.dart';
import '../../core/auth/session_controller.dart';
import '../../core/models.dart';
import '../../core/theme.dart';
import '../../core/ui/common.dart';

/// Photos preuve d'un PV : galerie + ajout (camera/galerie) + suppression.
/// Les images transitent par l'API authentifiée (object storage côté serveur).
class PvPhotosSection extends StatefulWidget {
  const PvPhotosSection({
    super.key,
    required this.controller,
    required this.pvId,
    this.editable = true,
  });

  final SessionController controller;
  final String pvId;
  final bool editable;

  @override
  State<PvPhotosSection> createState() => _PvPhotosSectionState();
}

class _PvPhotosSectionState extends State<PvPhotosSection> {
  final ImagePicker _picker = ImagePicker();
  late Future<List<PvPhoto>> _future = _load();
  bool _busy = false;
  String? _error;

  Future<List<PvPhoto>> _load() => widget.controller.pvPhotos(widget.pvId);

  void _reload() => setState(() => _future = _load());

  Future<void> _addPhoto(ImageSource source) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final file = await _picker.pickImage(
        source: source,
        imageQuality: 70,
        maxWidth: 1600,
      );
      if (file == null) {
        setState(() => _busy = false);
        return;
      }
      final bytes = await file.readAsBytes();
      await widget.controller.uploadPvPhoto(
        widget.pvId,
        bytes: bytes,
        filename: file.name,
        contentType: file.mimeType ?? _guessMime(file.name),
      );
      if (!mounted) return;
      _reload();
    } on ApiException catch (error) {
      setState(() => _error = error.message);
    } catch (_) {
      setState(() => _error = 'Ajout de la photo impossible');
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  Future<void> _delete(PvPhoto photo) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await widget.controller.deletePvPhoto(widget.pvId, photo.id);
      if (!mounted) return;
      _reload();
    } on ApiException catch (error) {
      setState(() => _error = error.message);
    } catch (_) {
      setState(() => _error = 'Suppression impossible');
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  String _guessMime(String name) {
    final lower = name.toLowerCase();
    if (lower.endsWith('.png')) return 'image/png';
    if (lower.endsWith('.webp')) return 'image/webp';
    if (lower.endsWith('.heic')) return 'image/heic';
    return 'image/jpeg';
  }

  @override
  Widget build(BuildContext context) {
    return SectionPanel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Expanded(
                child: Text(
                  'Photos preuve',
                  style: TextStyle(fontWeight: FontWeight.w900),
                ),
              ),
              if (_busy)
                const SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
            ],
          ),
          const SizedBox(height: 12),
          FutureBuilder<List<PvPhoto>>(
            future: _future,
            builder: (context, snapshot) {
              if (snapshot.connectionState != ConnectionState.done) {
                return const Padding(
                  padding: EdgeInsets.all(8),
                  child: Center(child: CircularProgressIndicator()),
                );
              }
              if (snapshot.hasError) {
                final message = snapshot.error is ApiException
                    ? (snapshot.error as ApiException).message
                    : 'Photos indisponibles';
                return Text(message, style: const TextStyle(color: apmRed));
              }
              final photos = snapshot.data ?? const [];
              if (photos.isEmpty) {
                return const Text(
                  'Aucune photo. Ajoutez une preuve ci-dessous.',
                  style: TextStyle(color: apmMuted),
                );
              }
              return GridView.count(
                crossAxisCount: 3,
                shrinkWrap: true,
                physics: const NeverScrollableScrollPhysics(),
                mainAxisSpacing: 8,
                crossAxisSpacing: 8,
                children: [
                  for (final photo in photos)
                    _PhotoTile(
                      url: widget.controller.photoContentUrl(
                        widget.pvId,
                        photo.id,
                      ),
                      headers: widget.controller.authHeaders,
                      onForbidden:
                          widget.controller.handleAuthenticatedAssetForbidden,
                      onDelete: widget.editable && !_busy
                          ? () => _delete(photo)
                          : null,
                    ),
                ],
              );
            },
          ),
          if (_error != null) ...[
            const SizedBox(height: 8),
            Text(
              _error!,
              style: const TextStyle(
                color: apmRed,
                fontWeight: FontWeight.w700,
              ),
            ),
          ],
          if (widget.editable) ...[
            const SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: OutlinedButton.icon(
                    onPressed: _busy
                        ? null
                        : () => _addPhoto(ImageSource.camera),
                    icon: const Icon(Icons.photo_camera_outlined),
                    label: const Text('Camera'),
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: OutlinedButton.icon(
                    onPressed: _busy
                        ? null
                        : () => _addPhoto(ImageSource.gallery),
                    icon: const Icon(Icons.photo_library_outlined),
                    label: const Text('Galerie'),
                  ),
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

class _PhotoTile extends StatelessWidget {
  const _PhotoTile({
    required this.url,
    required this.headers,
    required this.onForbidden,
    required this.onDelete,
  });

  final String url;
  final Map<String, String> headers;
  final VoidCallback onForbidden;
  final VoidCallback? onDelete;

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(8),
      child: Stack(
        fit: StackFit.expand,
        children: [
          Image.network(
            url,
            headers: headers,
            fit: BoxFit.cover,
            errorBuilder: (_, error, _) {
              if (error is NetworkImageLoadException &&
                  error.statusCode == 403) {
                onForbidden();
              }
              return Container(
                color: apmPanel,
                child: const Icon(Icons.broken_image_outlined, color: apmMuted),
              );
            },
            loadingBuilder: (context, child, progress) => progress == null
                ? child
                : Container(
                    color: apmPanel,
                    child: const Center(
                      child: SizedBox(
                        width: 18,
                        height: 18,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      ),
                    ),
                  ),
          ),
          Positioned(
            top: 2,
            right: 2,
            child: onDelete == null
                ? const SizedBox.shrink()
                : Material(
                    color: Colors.black54,
                    shape: const CircleBorder(),
                    child: IconButton(
                      iconSize: 16,
                      padding: const EdgeInsets.all(4),
                      constraints: const BoxConstraints(),
                      icon: const Icon(Icons.close, color: Colors.white),
                      onPressed: onDelete,
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}
