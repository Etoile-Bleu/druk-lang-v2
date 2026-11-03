# Druk Language : Référence Dzongkha Complète

> **Druk** (འབྲུག་) est un langage de programmation dont tous les mots-clés sont en **dzongkha**,
> la langue nationale du Bhoutan. Le script utilisé est le script tibétain (Uchen), commun aux deux langues.

---

## 1. Mots-clés implémentés

### Déclarations

| Mot-clé dzongkha | Romanisation | Signification | Équivalent |
|---|---|---|---|
| `ལས་འགན་` | *las 'gan* | "tâche, devoir" | `function` |
| `གྲངས་` | *grangs* | "nombre, chiffre" | `number` (int) |
| `ཡིག་འབྲུ་` | *yig 'bru* | "caractère, lettre" | `string` |
| `བདེན་རྫུན་` | *bden rdzun* | "vrai-faux" | `boolean` |

### Valeurs littérales

| Mot-clé dzongkha | Romanisation | Signification | Équivalent |
|---|---|---|---|
| `བདེན་པ་` | *bden pa* | "vérité" | `true` |
| `རྫུན་མ་` | *rdzun ma* | "fausseté" | `false` |

### Contrôle de flux

| Mot-clé dzongkha | Romanisation | Signification | Équivalent |
|---|---|---|---|
| `གལ་སྲིད་` | *gal srid* | "si possible, au cas où" | `if` |
| `མེད་ན་` | *med na* | "sinon, à défaut" | `else` |
| `ཡང་བསྐྱར་` | *yang bskyar* | "encore, de nouveau" | `while` |
| `རེ་རེར་` | *re rer* | "un par un, chacun" | `for` |
| `ཆད་` | *chad* | "interrompre" | `break` |
| `མུ་མཐུད་` | *mu mthud* | "continuer" | `continue` |
| `སླར་ལོག་` | *slar log* | "retourner, revenir" | `return` |

### Entrée / Sortie

| Mot-clé dzongkha | Romanisation | Signification | Équivalent |
|---|---|---|---|
| `བཀོད་` | *bkod* | "afficher, écrire" | `print` |

---

## 2. Fonctions intégrées (builtins)

Ces fonctions sont appelées comme des identifiants normaux.

| Identifiant | Signification | Équivalent |
|---|---|---|
| `རིང་ཚད` | *ring tshad* : "longueur, taille" | `len()` |
| `སྣོན་` | *snon* : "ajouter" | `push()` |
| `བཏོན་` | *bton* : "extraire, sortir" | `pop()` |
| `རིགས་` | *rigs* : "type, catégorie" | `typeof()` |
| `ལྡེ་མིག་` | *lde mig* : "clés" | `keys()` (pas encore implémenté) |
| `གནས་གོང་` | *gnas gong* : "valeurs" | `values()` (pas encore implémenté) |
| `ནང་འདུས་` | *nang 'dus* : "contient" | `contains()` (pas encore implémenté) |

Ces trois derniers dépendent d'un type dictionnaire que ce compilateur n'a pas encore ; les
appeler est une erreur de compilation claire (`druk check` les rejette lui-même, pas seulement
`druk run`/`druk build`), pas un plantage silencieux.
| `ནང་འཇུག་` | *nang 'jug* : "entrée" | `input()` |

---

## 3. Variables globales prédéfinies

| Variable dzongkha | Équivalent | Description |
|---|---|---|
| `ནང་འཇུག་ཐོ་` | `argv` | Arguments de la ligne de commande (tableau) |
| `ནང་འཇུག་གྲངས་` | `argc` | Nombre d'arguments |

---

## 4. Opérateurs

Tous les opérateurs utilisent des symboles ASCII standard.

| Opérateur | Description |
|---|---|
| `+` `-` `*` `/` `%` | Arithmétique entière (`%` : reste de la division) |
| `==` `!=` | Égalité |
| `<` `<=` `>` `>=` | Comparaison |
| `&&` `\|\|` | Logique ET / OU |
| `!` | Négation logique |
| `=` | Affectation |
| `.` | Accès membre (struct) |
| `[]` | Indexation (tableau) |

---

## 5. Types de données

| Type | Mot-clé | Description | Exemple |
|---|---|---|---|
| Entier | `གྲངས་` | 64-bit signé | `༡༢༣` |
| Chaîne | `ཡིག་འབྲུ་` | UTF-8 | `"བཀྲ་ཤིས་"` |
| Booléen | `བདེན་རྫུན་` | vrai/faux | `བདེན་པ་` |
| Tableau | *(littéral)* | Dynamique, GC | `[༡, ༢, ༣]` |
| Struct | *(littéral)* | Champs nommés, GC | `{clé: valeur}` |
| Nil | `ཅི་མེད` | Valeur nulle | N/A |

---

## 6. Numéraux tibétains / dzongkha

Les littéraux numériques utilisent les chiffres tibétains (U+0F20–U+0F29).

| Chiffre | Valeur |
|---|---|
| `༠` | 0 |
| `༡` | 1 |
| `༢` | 2 |
| `༣` | 3 |
| `༤` | 4 |
| `༥` | 5 |
| `༦` | 6 |
| `༧` | 7 |
| `༨` | 8 |
| `༩` | 9 |

Les nombres multi-chiffres s'écrivent normalement : `༡༢༣` = 123, `༡༠༠༠` = 1000.

---

## 7. Syntaxe complète : Exemples

### Déclaration de variable
```
གྲངས་ ཨང་ = ༡༠;
ཡིག་འབྲུ་ མིང་ = "བཀྲ་ཤིས་";
བདེན་རྫུན་ ཡིན་མིན་ = བདེན་པ་;
```

### Condition
```
གལ་སྲིད་ (ཨང་ > ༠) {
    བཀོད་ "ཡར་རྒྱས་";
} མེད་ན་ {
    བཀོད་ "མར་འབབ་";
}
```

### Boucle while
```
གྲངས་ ཨང་ = ༠;
ཡང་བསྐྱར་ (ཨང་ < ༥) {
    བཀོད་ ཨང་;
    ཨང་ = ཨང་ + ༡;
}
```

### Boucle for
```
རེ་རེར་ (གྲངས་ ཨང་ = ༠; ཨང་ < ༥; ཨང་ = ཨང་ + ༡) {
    བཀོད་ ཨང་;
}
```

### break et continue
```
གྲངས་ ཨང་ = ༠;
ཡང་བསྐྱར་ (བདེན་པ་) {
    ཨང་ = ཨང་ + ༡;
    གལ་སྲིད་ (ཨང་ == ༣) {
        མུ་མཐུད་;
    }
    གལ་སྲིད་ (ཨང་ > ༥) {
        ཆད་;
    }
    བཀོད་ ཨང་;
}
```

### Fonction
```
ལས་འགན་ བསྡོམས་ (གྲངས་ ཀ་, གྲངས་ ཁ་) {
    སླར་ལོག་ ཀ་ + ཁ་;
}
བཀོད་ བསྡོམས་(༣, ༤);
```

Le mot-clé de type sur un paramètre est optionnel : `druk-hir` déduit le type à partir de son
usage dans le corps de la fonction (une addition implique un nombre, une condition `if` implique
un booléen, etc.) :
```
ལས་འགན་ བསྡོམས་ (ཀ་, ཁ་) {
    སླར་ལོག་ ཀ་ + ཁ་;
}
```
Si l'usage ne permet pas de déduire un type sans ambiguïté (paramètre jamais utilisé, ou utilisé
de façons contradictoires), le compilateur demande d'ajouter le mot-clé de type explicitement sur
ce paramètre précis plutôt que d'échouer avec une erreur générique.

### Tableau
```
གྲངས་ ཐོ་ཡིག་ = [༡, ༢, ༣, ༤, ༥];
བཀོད་ རིང་ཚད(ཐོ་ཡིག་);
སྣོན་(ཐོ་ཡིག་, ༦);
```

### Struct
```
གྲངས་ མི་ = {མིང་: "རྡོ་རྗེ་", ལོ་: ༢༥};
བཀོད་ མི་.མིང་;
```

---

## 8. Mots-clés futurs (non encore implémentés)

Ces termes sont réservés pour de futures extensions du langage.

| Concept | Dzongkha proposé | Romanisation | Équivalent |
|---|---|---|---|
| `import` | `ནང་འདྲེན་` | *nang 'dren* | Importer un module |
| `export` | `ཕྱིར་འདྲེན་` | *phyir 'dren* | Exporter un symbole |
| `class` / `struct type` | `རིགས་གཞི་` | *rigs gzhi* | Définir un type |
| `null` / `nil` | `ཅི་མེད` | *ci med* | Valeur nulle explicite |
| `try` | `ཚོད་ལྟ་` | *tshod lta* | Essayer (gestion d'erreur) |
| `catch` | `འཛིན་` | *'dzin* | Attraper une erreur |
| `throw` | `འཕེན་` | *'phen* | Lancer une erreur |
| `async` | `རིམ་མིན་` | *rim min* | Asynchrone |
| `await` | `སྒུག་` | *sgug* | Attendre |
| `match` / `switch` | `འགྲིག་པ་` | *'grig pa* | Correspondance de motif |
| `enum` | `གྲངས་རིམ་` | *grangs rim* | Énumération |
| `const` | `མི་འགྱུར་` | *mi 'gyur* | Constante |
| `type` (alias) | `རིགས་མིང་` | *rigs ming* | Alias de type |

---

## 9. Séparateurs et ponctuation

| Symbole | Usage |
|---|---|
| `;` | Fin d'instruction |
| `{ }` | Bloc de code |
| `( )` | Condition, paramètres |
| `[ ]` | Tableau littéral, indexation |
| `,` | Séparateur d'éléments |
| `:` | Séparateur clé-valeur (struct) |
| `.` | Accès membre |
| `//` | Commentaire ligne |

---

## 10. Encodage et fichiers

- **Extension** : `.druk`
- **Encodage** : UTF-8 (BOM optionnel)
- **Script** : Tibétain Uchen (Unicode bloc U+0F00–U+0FFF)
- **Chiffres** : Tibétains (U+0F20–U+0F29) ou ASCII (0–9)
