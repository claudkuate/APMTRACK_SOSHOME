import 'dart:async';
import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:http_parser/http_parser.dart';

import '../config.dart';
import '../models.dart';

abstract class ApmtrackApi {
  Future<AuthSession> login(String email, String password);
  Future<AuthSession> refresh(String refreshToken);
  Future<void> logout(String token, String refreshToken);

  /// Remplace le mot de passe de l'utilisateur connecte (sortie du provisionnement).
  Future<void> changePassword(
    String token,
    String currentPassword,
    String newPassword,
  );
  Future<MobileProfile> mobileMe(String token);
  Future<List<Intervention>> mobileInterventions(String token);
  Future<PatrouilleActive> activePatrouille(String token);
  Future<List<Patrouille>> mobilePatrouilles(String token);
  Future<Paginated<Pv>> pvs(String token, {int pageSize = 20});
  Future<Pv> createPv(String token, CreatePvPayload payload);
  Future<Pv> updatePv(String token, String pvId, CreatePvPayload payload);
  Future<String> pvQrSvg(String token, String pvId);
  Future<List<int>> pvPdfBytes(String token, String pvId);
  Future<List<PvPhoto>> listPvPhotos(String token, String pvId);
  Future<PvPhoto> uploadPvPhoto(
    String token,
    String pvId, {
    required List<int> bytes,
    required String filename,
    required String contentType,
  });
  Future<void> deletePvPhoto(String token, String pvId, String photoId);
  String photoContentUrl(String pvId, String photoId);
  String agentPhotoContentUrl(String agentId);
  Future<PvPublic> verifyPublicPv(String pvNumber);
  Future<void> recordPatrouillePosition(
    String token,
    String patrouilleId, {
    required double latitude,
    required double longitude,
    double? accuracyM,
  });
}

class ApiException implements Exception {
  ApiException(this.message, {this.statusCode, this.code});

  final String message;
  final int? statusCode;
  final String? code;

  bool get isUnauthorized => statusCode == 401;
  bool get isCommuneSubscriptionInactive =>
      statusCode == 403 && code == 'COMMUNE_SUBSCRIPTION_INACTIVE';

  @override
  String toString() => message;
}

class HttpApmtrackApi implements ApmtrackApi {
  HttpApmtrackApi({http.Client? client, String? baseUrl})
    : _client = client ?? http.Client(),
      _baseUrl = (baseUrl ?? apiBaseUrl).replaceFirst(RegExp(r'/$'), '');

  final http.Client _client;
  final String _baseUrl;

  Uri _uri(String path, [Map<String, String?> query = const {}]) {
    final filtered = <String, String>{};
    for (final entry in query.entries) {
      final value = entry.value;
      if (value != null && value.isNotEmpty) {
        filtered[entry.key] = value;
      }
    }
    return Uri.parse('$_baseUrl$path').replace(queryParameters: filtered);
  }

  Map<String, String> _headers([String? token]) => {
    'content-type': 'application/json',
    if (token != null) 'authorization': 'Bearer $token',
  };

  Future<http.Response> _send(Future<http.Response> request) {
    return request.timeout(
      const Duration(seconds: 15),
      onTimeout: () => throw ApiException('API indisponible: delai depasse'),
    );
  }

  JsonMap _decodeObject(http.Response response) {
    final decoded = jsonDecode(response.body);
    if (decoded is Map<String, dynamic>) {
      return decoded;
    }
    throw ApiException('Reponse API invalide', statusCode: response.statusCode);
  }

  void _ensureOk(http.Response response) {
    if (response.statusCode >= 200 && response.statusCode < 300) {
      return;
    }

    var message = 'Erreur API ${response.statusCode}';
    String? code;
    try {
      final decoded = jsonDecode(response.body);
      if (decoded is Map<String, dynamic>) {
        final error = decoded['error'];
        if (error is Map<String, dynamic>) {
          message = error['message']?.toString() ?? message;
          code = error['code']?.toString();
        }
      }
    } catch (_) {
      if (response.body.trim().isNotEmpty) {
        message = response.body.trim();
      }
    }
    throw ApiException(
      message,
      statusCode: response.statusCode,
      code: code,
    );
  }

  @override
  Future<AuthSession> login(String email, String password) async {
    final response = await _send(
      _client.post(
        _uri('/api/v1/auth/login'),
        headers: _headers(),
        body: jsonEncode({'email': email, 'password': password}),
      ),
    );
    _ensureOk(response);
    return AuthSession.fromJson(_decodeObject(response));
  }

  @override
  Future<AuthSession> refresh(String refreshToken) async {
    final response = await _send(
      _client.post(
        _uri('/api/v1/auth/refresh'),
        headers: _headers(),
        body: jsonEncode({'refresh_token': refreshToken}),
      ),
    );
    _ensureOk(response);
    return AuthSession.fromJson(_decodeObject(response));
  }

  @override
  Future<void> logout(String token, String refreshToken) async {
    final response = await _send(
      _client.post(
        _uri('/api/v1/auth/logout'),
        headers: _headers(token),
        body: jsonEncode({'refresh_token': refreshToken}),
      ),
    );
    _ensureOk(response);
  }

  @override
  Future<void> changePassword(
    String token,
    String currentPassword,
    String newPassword,
  ) async {
    final response = await _send(
      _client.post(
        _uri('/api/v1/auth/change-password'),
        headers: _headers(token),
        body: jsonEncode({
          'current_password': currentPassword,
          'new_password': newPassword,
        }),
      ),
    );
    _ensureOk(response);
  }

  @override
  Future<MobileProfile> mobileMe(String token) async {
    final response = await _send(
      _client.get(_uri('/api/v1/mobile/me'), headers: _headers(token)),
    );
    _ensureOk(response);
    return MobileProfile.fromJson(_decodeObject(response));
  }

  @override
  Future<List<Intervention>> mobileInterventions(String token) async {
    final response = await _send(
      _client.get(
        _uri('/api/v1/mobile/interventions'),
        headers: _headers(token),
      ),
    );
    _ensureOk(response);
    final decoded = jsonDecode(response.body);
    if (decoded is! List) {
      throw ApiException('Reponse interventions invalide');
    }
    return decoded
        .whereType<Map<String, dynamic>>()
        .map(Intervention.fromJson)
        .toList();
  }

  @override
  Future<PatrouilleActive> activePatrouille(String token) async {
    final response = await _send(
      _client.get(
        _uri('/api/v1/mobile/patrouille-active'),
        headers: _headers(token),
      ),
    );
    _ensureOk(response);
    return PatrouilleActive.fromJson(_decodeObject(response));
  }

  @override
  Future<List<Patrouille>> mobilePatrouilles(String token) async {
    final response = await _send(
      _client.get(_uri('/api/v1/mobile/patrouilles'), headers: _headers(token)),
    );
    _ensureOk(response);
    final decoded = jsonDecode(response.body);
    if (decoded is! List) {
      throw ApiException('Reponse patrouilles invalide');
    }
    return decoded
        .whereType<Map<String, dynamic>>()
        .map(Patrouille.fromJson)
        .toList();
  }

  @override
  Future<Paginated<Pv>> pvs(String token, {int pageSize = 20}) async {
    final response = await _send(
      _client.get(
        _uri('/api/v1/pvs', {'page_size': pageSize.toString()}),
        headers: _headers(token),
      ),
    );
    _ensureOk(response);
    return Paginated.fromJson(_decodeObject(response), Pv.fromJson);
  }

  @override
  Future<Pv> createPv(String token, CreatePvPayload payload) async {
    final response = await _send(
      _client.post(
        _uri('/api/v1/pvs'),
        headers: _headers(token),
        body: jsonEncode(payload.toJson()),
      ),
    );
    _ensureOk(response);
    return Pv.fromJson(_decodeObject(response));
  }

  @override
  Future<Pv> updatePv(
    String token,
    String pvId,
    CreatePvPayload payload,
  ) async {
    final response = await _send(
      _client.patch(
        _uri('/api/v1/pvs/$pvId'),
        headers: _headers(token),
        body: jsonEncode(payload.toJson()),
      ),
    );
    _ensureOk(response);
    return Pv.fromJson(_decodeObject(response));
  }

  @override
  Future<String> pvQrSvg(String token, String pvId) async {
    final response = await _send(
      _client.get(_uri('/api/v1/pvs/$pvId/qr'), headers: _headers(token)),
    );
    _ensureOk(response);
    return response.body;
  }

  @override
  Future<List<int>> pvPdfBytes(String token, String pvId) async {
    final response = await _send(
      _client.get(_uri('/api/v1/pvs/$pvId/pdf'), headers: _headers(token)),
    );
    _ensureOk(response);
    return response.bodyBytes;
  }

  @override
  Future<List<PvPhoto>> listPvPhotos(String token, String pvId) async {
    final response = await _send(
      _client.get(_uri('/api/v1/pvs/$pvId/photos'), headers: _headers(token)),
    );
    _ensureOk(response);
    final decoded = jsonDecode(response.body);
    if (decoded is! List) {
      throw ApiException('Reponse photos invalide');
    }
    return decoded
        .whereType<Map<String, dynamic>>()
        .map(PvPhoto.fromJson)
        .toList();
  }

  @override
  Future<PvPhoto> uploadPvPhoto(
    String token,
    String pvId, {
    required List<int> bytes,
    required String filename,
    required String contentType,
  }) async {
    final request =
        http.MultipartRequest('POST', _uri('/api/v1/pvs/$pvId/photos'))
          ..headers['authorization'] = 'Bearer $token'
          ..files.add(
            http.MultipartFile.fromBytes(
              'file',
              bytes,
              filename: filename,
              contentType: MediaType.parse(contentType),
            ),
          );
    final streamed = await _client
        .send(request)
        .timeout(
          const Duration(seconds: 30),
          onTimeout: () =>
              throw ApiException('API indisponible: delai depasse'),
        );
    final response = await http.Response.fromStream(streamed);
    _ensureOk(response);
    return PvPhoto.fromJson(_decodeObject(response));
  }

  @override
  Future<void> deletePvPhoto(String token, String pvId, String photoId) async {
    final response = await _send(
      _client.delete(
        _uri('/api/v1/pvs/$pvId/photos/$photoId'),
        headers: _headers(token),
      ),
    );
    _ensureOk(response);
  }

  @override
  String photoContentUrl(String pvId, String photoId) =>
      '$_baseUrl/api/v1/pvs/$pvId/photos/$photoId';

  @override
  String agentPhotoContentUrl(String agentId) =>
      '$_baseUrl/api/v1/agents/$agentId/photo';

  @override
  Future<PvPublic> verifyPublicPv(String pvNumber) async {
    final encoded = Uri.encodeComponent(pvNumber.trim());
    final response = await _send(
      _client.get(_uri('/api/v1/public/pvs/$encoded')),
    );
    _ensureOk(response);
    return PvPublic.fromJson(_decodeObject(response));
  }

  @override
  Future<void> recordPatrouillePosition(
    String token,
    String patrouilleId, {
    required double latitude,
    required double longitude,
    double? accuracyM,
  }) async {
    final response = await _send(
      _client.post(
        _uri('/api/v1/patrouilles/$patrouilleId/positions'),
        headers: _headers(token),
        body: jsonEncode({
          'latitude': latitude,
          'longitude': longitude,
          'accuracy_m': accuracyM,
        }),
      ),
    );
    _ensureOk(response);
  }
}
