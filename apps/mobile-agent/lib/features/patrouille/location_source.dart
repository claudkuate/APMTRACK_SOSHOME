import 'package:geolocator/geolocator.dart';

import '../../core/api/api_client.dart';

/// A single GPS reading, decoupled from the geolocator `Position` type so the
/// tracker can be unit-tested without a real device.
class GpsFix {
  const GpsFix({
    required this.latitude,
    required this.longitude,
    this.accuracyM,
  });

  final double latitude;
  final double longitude;
  final double? accuracyM;
}

/// Source of GPS fixes. The real implementation wraps `geolocator`; tests
/// provide a fake that emits scripted fixes.
abstract class LocationSource {
  /// Ensures the location service is enabled and permission granted.
  /// Throws [ApiException] with a user-facing message otherwise.
  Future<void> ensureReady();

  /// Emits a fix every time the device moves at least [distanceFilterM] metres.
  Stream<GpsFix> watch({required int distanceFilterM});

  /// One-shot current position (used by manual capture).
  Future<GpsFix> current();
}

class GeolocatorLocationSource implements LocationSource {
  const GeolocatorLocationSource();

  @override
  Future<void> ensureReady() async {
    final serviceEnabled = await Geolocator.isLocationServiceEnabled();
    if (!serviceEnabled) {
      throw ApiException('GPS desactive sur le telephone');
    }
    var permission = await Geolocator.checkPermission();
    if (permission == LocationPermission.denied) {
      permission = await Geolocator.requestPermission();
    }
    if (permission == LocationPermission.denied ||
        permission == LocationPermission.deniedForever) {
      throw ApiException('GPS refuse');
    }
  }

  @override
  Stream<GpsFix> watch({required int distanceFilterM}) {
    return Geolocator.getPositionStream(
      locationSettings: LocationSettings(
        accuracy: LocationAccuracy.high,
        distanceFilter: distanceFilterM,
      ),
    ).map(_toFix);
  }

  @override
  Future<GpsFix> current() async {
    final position = await Geolocator.getCurrentPosition(
      locationSettings: const LocationSettings(
        accuracy: LocationAccuracy.high,
        timeLimit: Duration(seconds: 12),
      ),
    );
    return _toFix(position);
  }

  GpsFix _toFix(Position position) => GpsFix(
    latitude: position.latitude,
    longitude: position.longitude,
    accuracyM: position.accuracy,
  );
}
