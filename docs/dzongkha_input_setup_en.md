# Reading and writing Dzongkha: font and keyboard setup

> *[Version française](dzongkha_input_setup.md)*

> This guide answers a very concrete question: **how do I type and read `.druk` files
> correctly**, given that every keyword is written in Dzongkha (Tibetan Uchen script, Unicode
> block U+0F00–U+0FFF)? It's aimed at students and teachers using Druk in the classroom, not at
> compiler contributors.

## Why this isn't trivial

Tibetan/Dzongkha is written with **stacked consonants** (a "subjoined" consonant sitting under
another one, for example in `ལས་འགན་` or `གྲངས་`). Unicode encodes each letter separately, and
it's up to the rendering engine (and the font) to stack them visually. Two fonts that both claim
to "support Tibetan" don't necessarily produce the same result on the same Unicode sequence: some
correctly stack complex subjoined consonants, others render the letters side by side, disconnected,
which makes the text unreadable even though it's perfectly valid Unicode. This is a documented,
well-known problem, not a quirk of this project: see for example [this post comparing several
Tibetan fonts on the same
stacks](https://www.preining.info/blog/2019/10/pleasures-of-tibetan-input-and-typesetting-with-tex/).

In practice: if a `.druk` file renders as loose, disconnected letters instead of clean stacked
blocks, that's **not a mistake in your file**, it's the font or the keyboard setup that needs
fixing. This guide covers both.

## 1. Pick a font

| Font | Why | License | Where to get it |
|---|---|---|---|
| **Noto Sans Tibetan** (recommended) | Google's font, actively maintained, renders complex stacks well, available on every platform | SIL Open Font License 1.1 (free, reuse permitted) | [Google Fonts](https://fonts.google.com/noto/specimen/Noto+Sans+Tibetan) or the [`notofonts/tibetan` GitHub repo](https://github.com/notofonts/tibetan/releases) |
| **Microsoft Himalaya** | Already installed on Windows (Vista onward, including Windows 11): nothing to install if you're on Windows | Bundled with Windows; use is conditioned on your Windows license (don't redistribute it separately) | Already on your Windows machine |
| **BabelStone Tibetan** / **Jomolhari** | Historical alternatives, still functional and widely used (Jomolhari was Wikipedia's default Tibetan font for a long time, before being replaced by BabelStone Tibetan); worth trying if Noto doesn't fit a specific use case | SIL Open Font License | [THL's dedicated Jomolhari page](https://collab.its.virginia.edu/wiki/tibetan-script/Jomolhari.html) |
| **XTashi** | A Bhutanese font, designed for Dzongkha specifically (Chris Fynn and Pema Choden, for Bhutan's Department of Information Technology) | Free | [SALRC – XTashi](https://salrc.uchicago.edu/resources/fonts/available/tibetan/xtashi.shtml) |

**Recommendation: install Noto Sans Tibetan in your code editor and your terminal.** It's the most
actively maintained font and it correctly covers complex stacks. If you're on Windows and don't
want to install anything, Microsoft Himalaya (already present) is enough to read Dzongkha
correctly, even though it isn't meant to be redistributed.

Avoid fonts from the **Qomolangma** family for this project: they exist and render Tibetan well,
but their license forbids commercial use, which doesn't fit an open repository like this one.

### How to install a font (quick reminder)

- **Windows**: download the `.ttf`/`.otf` file, right-click → *Install*, then select that font in
  your editor's/terminal's font settings.
- **macOS**: Unicode Tibetan fonts are already preinstalled (Apple has included basic Tibetan
  rendering since Mac OS X 10.5), you normally don't need to do anything.
- **Linux**: copy the font file into `~/.local/share/fonts/` (or `/usr/share/fonts/` for all
  users), then run `fc-cache -f` so the system picks it up.

## 2. Activate a Dzongkha/Tibetan keyboard

Dzongkha and standard Tibetan (the one used in China/Tibet) share **the same Unicode block**
(U+0F00–U+0FFF): a "Tibetan" keyboard types into the same character space as Dzongkha. What
differs is **the key layout** (which key produces which letter), not the final Unicode result. The
steps below therefore give, for each system, both the simplest built-in keyboard to activate and,
where it exists, the Dzongkha keyboard standardized by the Bhutanese government, if you'd rather
have a layout designed for Dzongkha specifically.

### Windows

1. **Settings** → **Time & Language** → **Language & region**.
2. In the language list, click **⋯** next to your main language → **Language options**, then
   **Add a keyboard**.
3. Select **Tibetan (PRC)** from the list. This is the Tibetan keyboard built into Windows since
   Windows 10 — it writes into the same Unicode block as Dzongkha.
4. Switch between keyboards with **Win + Space**, or via the language icon in the taskbar.

> **A real caveat worth knowing**: the "Tibetan (PRC)" keyboard follows the standard Chinese
> layout for Tibetan, not the official Bhutanese Dzongkha layout. The text it produces is
> perfectly valid Dzongkha Unicode, but the position of the letters on the keyboard won't match
> what a physical Bhutanese keyboard would show. If you want the official Dzongkha layout
> (standardized in 2000 by the *Dzongkha Development Commission* and Bhutan's Department of
> Information Technology, revised in 2009), install the dedicated keyboard from the [Dzongkha
> Unicode Keyboard page on the THL
> wiki](https://collab.its.virginia.edu/wiki/tibetan-script/Dzongkha%20Unicode%20Keyboard.html).
> The instructions on that page date from the Windows XP era, but the keyboard file built with
> MSKLC still works on current versions of Windows.
>
> The exact menu (names, number of clicks) can vary slightly depending on the exact build of
> Windows 11 you're running: if a step doesn't quite match what you see, search for "add a
> keyboard" in the Settings search bar, which will take you straight to the right screen.

### macOS

1. **System Settings** → **Keyboard** → **Text Input** (or **Input Sources** on older versions of
   macOS) → **Edit...** → **+**.
2. Search for **Tibetan**, then pick a variant: **Tibetan - Wylie** (the most common one, Latin
   transliteration into Tibetan), **Tibetan - QWERTY**, or **Tibetan - Otani**. All three keyboards
   have shipped with macOS since Mac OS X 10.5 (Leopard).
3. Switch input sources with **⌘ + Space** (previous source) or **⌥ + ⌘ + Space** (next source),
   or via the flag icon in the menu bar.

> Unicode Tibetan fonts are already included in macOS: no font installation is needed on a Mac.
>
> The menu names above match recent versions of macOS (Ventura/Sonoma and later); on an older
> version, the path is **System Preferences** → **Keyboard** → **Input Sources**.

### Linux (GNOME / ibus)

Two possible approaches, depending on what you're after:

**A. Wylie input (Latin transliteration → Tibetan), via ibus:**

1. Install `ibus` and `ibus-m17n` (e.g. `sudo apt install ibus ibus-m17n` on Debian/Ubuntu).
2. Restart your session (or restart the ibus daemon).
3. **Settings** → **Keyboard** → **Input Sources** → **+**, then search for **Tibetan EWTS
   (m17n)**. Make sure you pick the *EWTS* variant, not the older standalone "Wylie" entry, which
   is outdated.

**B. Native keyboard layout (XKB), without going through ibus:**

A Tibetan layout exists directly in X11/XKB. To activate it from the command line:

```sh
setxkbmap -layout cn -variant tib
```

> **Worth checking before using this command in class**: the Tibetan variant has historically
> lived inside the `cn` (Chinese) symbols file under the name `tib`, rather than in its own
> `bo`/`bod` file — that's a documented maintenance choice by the xkeyboard-config developers, not
> a mistake in this guide. List the variants actually available on your machine before typing the
> command:
>
> ```sh
> localectl list-x11-keymap-variants cn
> ```
>
> A Dzongkha keyboard layout specific to Bhutan (standardized by the Bhutanese government) has also
> been merged into xkeyboard-config; if your distribution is up to date, look for a Bhutan-related
> variant (`bt`) in the list instead of `cn`/`tib`. Exact variant names can differ between
> distributions: always use `localectl list-x11-keymap-variants` to see what's actually installed
> rather than trusting a name found online.

4. Install a Tibetan font (see section 1) and run `fc-cache -f` if it doesn't show up immediately
   in your editor.

## 3. Checking that everything works

After installing a font and activating a keyboard, open one of the repo's examples in your
editor:

```sh
cat examples/01_hello.druk
```

You should see clean, stacked blocks of letters, not a string of disjointed little characters or
empty boxes (`□`). Keywords like `ལས་འགན་` (function) or `གྲངས་` (number) each contain a subjoined
consonant stacked under another one: that's exactly the kind of sequence a misconfigured
font/renderer gets wrong. If this text displays cleanly in your editor and your terminal, your
setup is good.

You can also type this test yourself directly into a new file, using the keyboard you just
activated, to check that you can produce the same glyphs already present in the repo:

```druk
བཀོད་ "Tashi Delek";
```

If this line displays and runs correctly with `cargo run -p druk -- run` (see the README for build
instructions), both your font and your keyboard are correctly set up.

## 4. Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Letters display separated, not stacked | Font without good support for complex Tibetan stacks, or the editor isn't using the font you installed | Install Noto Sans Tibetan and select it explicitly in your editor's/terminal's font settings |
| Empty boxes (`□`) or question marks instead of letters | No Tibetan font installed on the system | Install a font from section 1 |
| The keyboard you added doesn't produce any Tibetan character | Wrong variant selected (e.g. a Latin keyboard instead of "Tibetan (PRC)"/"EWTS") | Double-check the exact keyboard name chosen in step 2 |
| On Linux, `setxkbmap` returns an error | The `tib` variant doesn't exist under that name on your distribution | Run `localectl list-x11-keymap-variants cn` to see the names actually available, or use the ibus/m17n route instead |

## Sources and further reading

- [DigitalTibetan](https://digitaltibetan.github.io/DigitalTibetan/main.html) — the most complete
  and best-structured guide found on this topic (fonts, Windows, Linux GNOME/KDE, macOS, a
  detailed explanation of rendering problems). This document's structure is directly inspired by
  it.
- [THL (Tibetan and Himalayan Library) – Tibetan script
  resources](https://collab.its.virginia.edu/wiki/tibetan-script/) — the historical source cited by
  this guide's originating issue, notably for the official Dzongkha keyboard.
- ["Pleasures of Tibetan input and typesetting with
  TeX"](https://www.preining.info/blog/2019/10/pleasures-of-tibetan-input-and-typesetting-with-tex/)
  — concretely explains the consonant-stack rendering problem that motivates this guide.
- [Dzongkha keyboard layout — Wikipedia](https://en.wikipedia.org/wiki/Dzongkha_keyboard_layout) —
  history of the official Bhutanese keyboard layout (2000, revised in 2009).
- [X Input Method for Dzongkha Script (Bhutan's Department of Information
  Technology)](https://tech.gov.bt/sites/default/files/X_Input_Method_for_Dzongkha_Script.pdf) —
  the technical document behind the government keyboard layout.
