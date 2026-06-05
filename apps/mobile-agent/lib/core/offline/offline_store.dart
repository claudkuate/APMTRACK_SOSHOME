import 'package:flutter_secure_storage/flutter_secure_storage.dart';

/// Persistent key/value store for the offline read cache and the PV draft queue.
/// Backed by encrypted secure storage (the cache holds PII: names, plates).
abstract class OfflineCacheStore {
  Future<String?> read(String key);
  Future<void> write(String key, String value);
  Future<void> delete(String key);
}

class SecureOfflineCacheStore implements OfflineCacheStore {
  SecureOfflineCacheStore({FlutterSecureStorage? storage})
    : _storage = storage ?? const FlutterSecureStorage();

  final FlutterSecureStorage _storage;

  @override
  Future<String?> read(String key) => _storage.read(key: key);

  @override
  Future<void> write(String key, String value) =>
      _storage.write(key: key, value: value);

  @override
  Future<void> delete(String key) => _storage.delete(key: key);
}

class MemoryOfflineCacheStore implements OfflineCacheStore {
  final Map<String, String> _data = {};

  @override
  Future<String?> read(String key) async => _data[key];

  @override
  Future<void> write(String key, String value) async => _data[key] = value;

  @override
  Future<void> delete(String key) async => _data.remove(key);
}
