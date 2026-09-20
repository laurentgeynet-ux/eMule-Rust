import 'package:flutter/widgets.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// UI locale preference (independent from daemon settings).
///
/// Defaults to French and persists the choice locally via
/// `shared_preferences` under the `app.locale` key.
class LocaleController extends ChangeNotifier {
  static const _prefKey = 'app.locale';

  Locale _locale = const Locale('fr');

  Locale get locale => _locale;

  /// Restores the persisted locale, if any.
  Future<void> load() async {
    final prefs = await SharedPreferences.getInstance();
    final code = prefs.getString(_prefKey);
    if (code != null && code != _locale.languageCode) {
      _locale = Locale(code);
      notifyListeners();
    }
  }

  Future<void> setLocale(Locale locale) async {
    if (locale == _locale) return;
    _locale = locale;
    notifyListeners();
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_prefKey, locale.languageCode);
  }
}
