package i18n

import "testing"

func TestTranslationsHaveEnglishAndChinese(t *testing.T) {
	for key, values := range translations {
		if values[EN] == "" {
			t.Fatalf("%s missing English translation", key)
		}
		if values[ZH] == "" {
			t.Fatalf("%s missing Chinese translation", key)
		}
	}
}
