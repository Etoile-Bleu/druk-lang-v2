# Lire et écrire le dzongkha : police et clavier

> *[English version](dzongkha_input_setup_en.md)*

> Ce guide répond à une question très concrète : **comment taper et relire correctement les
> fichiers `.druk`**, dont tous les mots-clés sont en dzongkha (écriture tibétaine Uchen,
> bloc Unicode U+0F00–U+0FFF). Il s'adresse aux élèves et aux enseignants qui utilisent Druk en
> classe, pas aux contributeurs du compilateur.

## Pourquoi ce n'est pas trivial

Le tibétain/dzongkha s'écrit avec des **piles de consonnes** (une consonne « souscrite » sous une
autre, par exemple dans `ལས་འགན་` ou `གྲངས་`). Unicode encode chaque lettre séparément et c'est au
moteur de rendu (et à la police) de les empiler visuellement. Deux polices qui prétendent toutes
les deux « supporter le tibétain » ne donnent pas forcément le même résultat sur la même séquence
Unicode : certaines empilent correctement les consonnes souscrites complexes, d'autres affichent
les lettres côte à côte, séparées, ce qui rend le texte illisible même s'il est parfaitement valide
en Unicode. C'est un problème documenté et bien connu, pas une bizarrerie de ce projet : voir par
exemple [ce billet qui compare plusieurs polices tibétaines sur les mêmes
piles](https://www.preining.info/blog/2019/10/pleasures-of-tibetan-input-and-typesetting-with-tex/).

Concrètement : si un fichier `.druk` s'affiche avec des lettres décousues plutôt que des blocs
empilés propres, ce n'est **pas une erreur de votre fichier**, c'est la police ou la configuration
clavier qu'il faut corriger. Ce guide couvre les deux.

## 1. Choisir une police

| Police | Pourquoi | Licence | Où l'avoir |
|---|---|---|---|
| **Noto Sans Tibetan** (recommandée) | Police de Google, activement maintenue, bon rendu des piles complexes, disponible sur toutes les plateformes | SIL Open Font License 1.1 (libre, réutilisation permise) | [Google Fonts](https://fonts.google.com/noto/specimen/Noto+Sans+Tibetan) ou [dépôt GitHub `notofonts/tibetan`](https://github.com/notofonts/tibetan/releases) |
| **Microsoft Himalaya** | Déjà installée sur Windows (Vista et versions suivantes, y compris Windows 11) : rien à installer si vous êtes sous Windows | Incluse avec Windows ; usage conditionné à la licence Windows (ne pas la redistribuer séparément) | Déjà sur votre machine Windows |
| **BabelStone Tibetan** / **Jomolhari** | Alternatives historiques, toujours fonctionnelles et largement utilisées (Jomolhari a longtemps été la police par défaut de Wikipédia pour le tibétain, avant d'être remplacée par BabelStone Tibetan) ; à essayer si Noto ne convient pas pour un usage précis | SIL Open Font License | [Page THL dédiée à Jomolhari](https://collab.its.virginia.edu/wiki/tibetan-script/Jomolhari.html) |
| **XTashi** | Police bhoutanaise, conçue pour le dzongkha spécifiquement (Chris Fynn et Pema Choden, pour le Department of Information Technology du Bhoutan) | Gratuite | [SALRC – XTashi](https://salrc.uchicago.edu/resources/fonts/available/tibetan/xtashi.shtml) |

**Recommandation : installez Noto Sans Tibetan dans votre éditeur de code et votre terminal.**
C'est la police la plus largement maintenue et elle couvre correctement les piles complexes. Si
vous êtes sous Windows et ne voulez rien installer, Microsoft Himalaya (déjà présente) suffit pour
lire du dzongkha correctement, même si elle n'est pas prévue pour être redistribuée.

Évitez les polices de la famille **Qomolangma** pour ce projet : elles existent et rendent bien le
tibétain, mais leur licence interdit un usage commercial, ce qui ne convient pas à un dépôt ouvert
comme celui-ci.

### Comment installer une police (rappel rapide)

- **Windows** : téléchargez le fichier `.ttf`/`.otf`, clic droit → *Installer*, puis sélectionnez
  cette police dans les paramètres de police de votre éditeur/terminal.
- **macOS** : les polices tibétaines Unicode sont déjà préinstallées (Apple inclut un rendu
  tibétain de base depuis Mac OS X 10.5), vous n'avez normalement rien à faire.
- **Linux** : copiez le fichier de police dans `~/.local/share/fonts/` (ou `/usr/share/fonts/` pour
  tous les utilisateurs), puis lancez `fc-cache -f` pour que le système la détecte.

## 2. Activer un clavier dzongkha/tibétain

Le dzongkha et le tibétain standard (celui utilisé en Chine/Tibet) partagent **le même bloc
Unicode** (U+0F00–U+0FFF) : un clavier « Tibetan » tapera dans le même espace de caractères que le
dzongkha. Ce qui diffère, c'est **la disposition des touches** (quelle touche produit quelle
lettre), pas le résultat final en Unicode. Les étapes ci-dessous donnent donc, pour chaque
système, à la fois le clavier intégré le plus simple à activer et, quand il existe, le clavier
dzongkha standardisé par le gouvernement bhoutanais si vous préférez une disposition pensée pour le
dzongkha en particulier.

### Windows

1. **Paramètres** → **Heure et langue** → **Langue et région**.
2. Dans la liste des langues, cliquez sur **⋯** à côté de votre langue principale → **Options de
   langue**, puis **Ajouter un clavier**.
3. Sélectionnez **Tibétain (RPC)** (*Tibetan (PRC)*) dans la liste. C'est le clavier tibétain
   intégré à Windows depuis Windows 10 — il écrit dans le même bloc Unicode que le dzongkha.
4. Basculez entre claviers avec **Win + Espace**, ou via l'icône de langue dans la barre des
   tâches.

> **Point d'attention réel** : le clavier « Tibétain (RPC) » suit la disposition standard chinoise
> pour le tibétain, pas la disposition officielle bhoutanaise du dzongkha. Le texte produit est un
> Unicode dzongkha parfaitement valide, mais l'emplacement des lettres sur le clavier ne
> correspondra pas à ce qu'un clavier physique bhoutanais afficherait. Si vous voulez la disposition
> dzongkha officielle (normalisée en 2000 par la *Dzongkha Development Commission* et le
> Department of Information Technology du Bhoutan, révisée en 2009), installez le clavier dédié
> depuis la page du [Dzongkha Unicode Keyboard sur le wiki
> THL](https://collab.its.virginia.edu/wiki/tibetan-script/Dzongkha%20Unicode%20Keyboard.html).
> Les instructions de cette page datent de l'époque Windows XP mais le fichier clavier généré avec
> MSKLC fonctionne toujours sur les versions récentes de Windows.
>
> Le menu exact (noms, nombre de clics) peut varier légèrement selon la version exacte de Windows
> 11 que vous utilisez : si une étape ne correspond pas pile à ce que vous voyez, cherchez
> « ajouter un clavier » dans la barre de recherche de Paramètres, qui vous amènera directement au
> bon écran.

### macOS

1. **Réglages Système** → **Clavier** → **Saisie de texte** (ou **Sources de saisie** sur des
   versions plus anciennes de macOS) → **Modifier...** → **+**.
2. Cherchez **Tibétain**, puis choisissez une variante : **Tibetan - Wylie** (la plus courante,
   translittération latine vers tibétain), **Tibetan - QWERTY**, ou **Tibetan - Otani**. Ces trois
   claviers sont fournis par macOS depuis Mac OS X 10.5 (Leopard).
3. Changez de source de saisie avec **⌘ + Espace** (source précédente) ou **⌥ + ⌘ + Espace**
   (source suivante), ou via le drapeau dans la barre de menus.

> Les polices tibétaines Unicode sont déjà incluses dans macOS : aucune installation de police
> n'est nécessaire sur Mac.
>
> Les noms de menus ci-dessus correspondent aux versions récentes de macOS (Ventura/Sonoma et
> suivantes) ; sur une version plus ancienne, le chemin est **Préférences Système** → **Clavier**
> → **Sources de saisie**.

### Linux (GNOME / ibus)

Deux approches possibles, selon ce que vous cherchez :

**A. Saisie Wylie (translittération latine → tibétain), via ibus :**

1. Installez `ibus` et `ibus-m17n` (ex. `sudo apt install ibus ibus-m17n` sur Debian/Ubuntu).
2. Redémarrez votre session (ou relancez le démon ibus).
3. **Paramètres** → **Clavier** → **Sources de saisie** → **+**, puis cherchez **Tibetan EWTS
   (m17n)**. Choisissez bien la variante *EWTS*, pas l'ancienne entrée « Wylie » toute seule, qui
   est obsolète.

**B. Disposition clavier native (XKB), sans passer par ibus :**

Une disposition tibétaine existe directement dans X11/XKB. Pour l'activer en ligne de commande :

```sh
setxkbmap -layout cn -variant tib
```

> **À vérifier avant d'utiliser cette commande en classe** : la variante tibétaine est historiquement
> rangée dans le fichier de symboles `cn` (chinois) sous le nom `tib`, plutôt que dans un fichier
> dédié `bo`/`bod` — c'est un choix de maintenance documenté par les développeurs de
> xkeyboard-config, pas une erreur de ce guide. Listez les variantes réellement disponibles sur
> votre machine avant de taper la commande :
>
> ```sh
> localectl list-x11-keymap-variants cn
> ```
>
> Une disposition clavier dzongkha spécifique au Bhoutan (normalisée par le gouvernement bhoutanais)
> a aussi été intégrée à xkeyboard-config ; si votre distribution est à jour, cherchez une variante
> liée au Bhoutan (`bt`) dans la liste plutôt que `cn`/`tib`. Les noms exacts de variantes peuvent
> changer d'une distribution à l'autre : utilisez toujours `localectl list-x11-keymap-variants`
> pour voir ce qui est réellement installé plutôt que de faire confiance à un nom trouvé en ligne.

4. Installez une police tibétaine (voir section 1) et lancez `fc-cache -f` si elle n'apparaît pas
   immédiatement dans votre éditeur.

## 3. Vérifier que tout fonctionne

Après avoir installé une police et activé un clavier, ouvrez un des exemples du dépôt dans votre
éditeur :

```sh
cat examples/01_hello.druk
```

Vous devez voir des blocs de lettres empilées, nettes, et non une suite de petits caractères
disjoints ou des carrés vides (`□`). Les mots-clés comme `ལས་འགན་` (fonction) ou `གྲངས་` (nombre)
contiennent chacun une consonne souscrite empilée sous une autre : c'est exactement le genre de
séquence qu'une police/un rendu mal configuré affiche de travers. Si ce texte s'affiche proprement
dans votre éditeur et votre terminal, votre configuration est bonne.

Vous pouvez aussi taper vous-même ce test directement dans un nouveau fichier, en utilisant le
clavier que vous venez d'activer, pour vérifier que vous arrivez à produire les mêmes glyphes que
ceux déjà présents dans le dépôt :

```druk
བཀོད་ "Tashi Delek";
```

Si cette ligne s'affiche et s'exécute correctement avec `cargo run -p druk -- run` (voir le README
pour la compilation), votre police et votre clavier sont tous les deux correctement configurés.

## 4. Dépannage

| Symptôme | Cause probable | Solution |
|---|---|---|
| Les lettres s'affichent séparées, non empilées | Police sans bon support des piles tibétaines complexes, ou éditeur qui n'utilise pas la police installée | Installez Noto Sans Tibetan et sélectionnez-la explicitement dans les paramètres de police de l'éditeur/terminal |
| Carrés vides (`□`) ou points d'interrogation à la place des lettres | Aucune police tibétaine installée sur le système | Installez une police de la section 1 |
| Le clavier ajouté ne produit aucun caractère tibétain | Mauvaise variante sélectionnée (ex. clavier latin au lieu de « Tibetan (PRC)' »/« EWTS ») | Revérifiez le nom exact du clavier choisi à l'étape 2 |
| Sous Linux, `setxkbmap` renvoie une erreur | La variante `tib` n'existe pas sous ce nom sur votre distribution | Lancez `localectl list-x11-keymap-variants cn` pour voir les noms réellement disponibles, ou utilisez la voie ibus/m17n à la place |

## Sources et pour aller plus loin

- [DigitalTibetan](https://digitaltibetan.github.io/DigitalTibetan/main.html) — le guide le plus
  complet et le mieux structuré trouvé sur le sujet (polices, Windows, Linux GNOME/KDE, macOS,
  explication détaillée des problèmes de rendu). La structure de ce document s'en inspire
  directement.
- [THL (Tibetan and Himalayan Library) – ressources sur le script
  tibétain](https://collab.its.virginia.edu/wiki/tibetan-script/) — source historique citée par
  l'issue d'origine de ce guide, notamment pour le clavier dzongkha officiel.
- [« Pleasures of Tibetan input and typesetting with
  TeX »](https://www.preining.info/blog/2019/10/pleasures-of-tibetan-input-and-typesetting-with-tex/)
  — explique concrètement le problème de rendu des piles de consonnes qui motive ce guide.
- [Dzongkha keyboard layout — Wikipédia](https://en.wikipedia.org/wiki/Dzongkha_keyboard_layout) —
  historique de la disposition clavier officielle bhoutanaise (2000, révisée en 2009).
- [X Input Method for Dzongkha Script (Department of Information Technology,
  Bhoutan)](https://tech.gov.bt/sites/default/files/X_Input_Method_for_Dzongkha_Script.pdf) —
  document technique à la source de la disposition clavier gouvernementale.
