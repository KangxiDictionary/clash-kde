import org.kde.ki18n as KI18n

KI18n.KI18nContext {
    translationDomain: "kcm_clashkde"

    // Grouping, separators, and the decimal mark come from the user's formats.
    function formatNumber(value, decimals) {
        const loc = Qt.locale();
        if (decimals === undefined || decimals === 0) {
            return loc.toString(Math.round(value));
        }
        return loc.toString(value, "f", decimals);
    }
}
