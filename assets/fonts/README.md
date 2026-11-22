# Embedded fonts

`Roboto-Regular.ttf` and `Roboto-Medium.ttf` are subsets (Basic Latin, Latin-1,
Latin Extended-A, `Ə`/`ə`, common punctuation) of Roboto, the typeface used by
Telegram for Android. Copyright 2015 Google Inc., licensed under the Apache
License 2.0 (see `LICENSE-Apache-2.0.txt`). Source: https://github.com/google/roboto

Regenerate with:

    python3 -m fontTools.subset Roboto-Regular.ttf --output-file=Roboto-Regular.ttf \
      --unicodes="U+0020-007E,U+00A0-017F,U+018F,U+0259,U+02BC,U+2013-2014,U+2018-201E,U+2022,U+2026,U+20AC,U+2192" \
      --layout-features=kern --no-hinting --desubroutinize
