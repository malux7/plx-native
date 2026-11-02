# Continue Watching multiserveur : fenêtre initiale

Investigation sur `fix-cw-multiserver`, base propre
`72de84aabf9c7d7d95bad388e58b7e2e96490424`. Aucun ancien correctif de focus ou de
scroll n'est repris. Le défaut de données ci-dessous est reproduit sur hôte.
L'utilisateur a ensuite confirmé la disparition du décalage sur sa LG OLED65G1
sous webOS 6.5, avec trois serveurs Plex et après fermeture complète puis réouverture.

## Deux positions différentes

La position visible combine deux états indépendants :

1. **La fenêtre de données** : `pms::project` construit chaque aperçu depuis
   `/hubs/continueWatching`, puis `with_lookahead` complète la lecture via
   `/hubs/continueWatching/items`. `step_landings_with_scope` reçoit les réponses
   indépendantes, appelle `settle_deck`, puis publie `merge_held`.
   `stores::deck::place` choisit les bornes `Lane.lo..hi` par serveur.
   `merge_deck` trie les cartes retenues par dernière lecture ; `merge_with_scope`
   filtre les bibliothèques épinglées, regroupe les séries avec `deck_group::present`
   et expose un `HubRef.offset` et ses cartes. Un offset positif signale des cartes
   avant la fenêtre, même si le scroll graphique vaut zéro. Ce n'est pas un offset
   exact après filtrage et regroupement.
2. **Le scroll en pixels dans cette fenêtre** : `HomeScreen::sync_catalog` projette
   les identités des hubs et des cartes ; une nouvelle `Shelf` contient un
   `CardRow` dont `scroll_x` commence à zéro. Les shelves existantes suivent leur
   identité, pas leur numéro de rangée. `HomeCards::global` vaut `hub.offset + i`,
   mais ce nombre n'est pas le scroll graphique.

Les autres chemins qui modifient le scroll graphique ont été examinés :

- `HomeScreen::restore` / `HomeMemory.row_scroll`, appliqués dans `sync_catalog` ;
  retour sur une page retenue ou remontée, avec les clés d'éléments mémorisées.
- `Focusable::reconcile`, les sièges et `HomeCmd::FocusGrid` choisissent un
  élément. `FocusMoved`, dont `By::Restore`, nourrit les shelves ; la restauration
  rend l'élément visible avec `scroll_into_view` et `restore_scroll`.
- `Shelf::tick` / `pool::slid` suivent un élément dont l'indice a changé lors
  d'une publication : déplacement du scroll et du pop pour conserver sa position.
- `CardRow::update` rapproche le scroll de `scroll_target` uniquement quand la
  rangée est focalisée et non en défilement libre. Sans focus, le scroll reste fixe.
- `InputKind::Pan` appelle `Shelf::glide` pour un déplacement horizontal au
  pointeur. La molette déplace le document vertical, pas la fenêtre de données.
- La pagination de `Shelf::page_ask` émet `HubsCmd::Page`; le store avance ou recule
  les lanes. Les rafraîchissements utilisent `deck::reload` et `fit` pour conserver
  une fenêtre volontairement parcourue.

La sérialisation `pms::initial` concerne le bootstrap d'un enregistrement/replay,
pas une restauration automatique du scroll Home au redémarrage ordinaire.

## Défaut reproduit

Avant correction, `place` assimilait « un premier serveur est déjà placé » à
« conserver une fenêtre parcourue ». Lorsqu'un autre serveur répondait, seules ses
cartes comprises dans la tranche chronologique du premier étaient montrées.
Ses cartes plus récentes se retrouvaient avant la fenêtre. Si le résultat
dépassait 24 cartes, `drop_excess(lanes, true)` supprimait en plus les plus récentes.

Exemples exécutés sur le code propre, avant modification de la logique :

```text
Serveur A : dates 30, 20, 10 ; serveur B tardif : 60, 50, 40
Obtenu   : A0 A1 A2
Attendu  : B0 B1 B2 A0 A1 A2

Deux serveurs de 24 cartes aux dates entrelacées
Obtenu   : B12 A12 ... B23 A23
Attendu  : B0  A0  ... B11 A11

test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 1453 filtered out
```

Deux tests portent sur les lanes (petite liste et dépassement de capacité, les
deux ordres d'arrivée) ; le troisième vérifie les cartes et l'offset publiés au
catalogue Home via `settle_deck(..., true)`, le chemin des réponses de serveurs.
Les journaux avant correction sont conservés dans
`/workspace/.plx-cloud/cw-regression-all-before.log` sur la machine de travail.

La correction distingue la tête réelle (`lo == 0` et aucun élément avant les
lanes déjà placées) d'une fenêtre avancée. Un serveur rejoignant la tête recalcule
les premières cartes de la fusion ; un serveur rejoignant une fenêtre avancée
garde la règle existante. Aucune remise forcée à zéro du focus ou du scroll Home.

Cela fournit une explication testable aux observations : remonter avec Gauche
peut demander les pages précédentes et retrouver la vraie tête ; l'attente seule
ne demande pas ces pages. Le redémarrage refait arriver les serveurs et peut
recréer la mauvaise fenêtre. Ce mécanisme ne prouve pas à lui seul que le scroll
en pixels de la TV n'a aucun autre défaut.

## Résultat de la validation TV

Le build instrumenté `0d0e4c13` a été construit par le
[run GitHub Actions 41506047988](https://github.com/malux7/plx-native/actions/runs/41506047988),
terminé avec succès (compilation ARM, paquet et contrôles du pipeline réutilisé).
L'utilisateur confirme que le problème ne revient plus après fermeture complète
et réouverture de l'application.

Les journaux fournis montrent les réponses successives des sources 1, 2 puis 0 :
la fenêtre fusionnée reste à sa tête (`before=false`, toutes les bornes `lo=0`).
Home publie ensuite `count=24 offset=0 scroll=0.0 focus=None`. Le premier mouvement
directionnel place le focus sur `(0, 0)` et les rafraîchissements ultérieurs
conservent l'offset et le scroll à zéro, sans déplacement `cwdiag: motion`.
La confirmation des fermetures et réouvertures vient du test manuel de
l'utilisateur ; une copie répétée du même journal ne compte pas comme un autre boot.

Les traces temporaires `cwdiag:` ont été retirées après cette confirmation.
Le build final sans instrumentation (`81d51568`) a ensuite passé le
[pipeline ARM 41512911790](https://github.com/malux7/plx-native/actions/runs/41512911790)
en 17 min 16 s ; l'IPK est dans l'artefact `cw-final-ipk`. Le workflow ponctuel
a été retiré après ce build. Le correctif et les trois tests de régression sont
conservés. Aucun journal privé de la TV n'est ajouté au dépôt.

## Protocole utilisé avec le build instrumenté

Le build `0d0e4c13` contenait des traces `cwdiag:` actives aussi dans une compilation
Nightly `RELEASE=1`, sans trigger. Elles ne changeaient aucun choix de focus et ne
contenaient ni adresse, ni titre, ni compte, ni jeton. Les instructions de collecte
ci-dessous documentent cette investigation ; une nouvelle compilation de la
branche nettoyée n'émet plus ces traces.

Pour construire depuis le workflow upstream : Actions → **Nightly** → **Run
workflow**, sélectionner `fix-cw-multiserver` et cocher **dry_run**. Ce mode est
nécessaire sur une branche : le workflow refuse une publication Nightly réelle
hors `main`. Télécharger l'IPK dans l'artefact `nightly-assets` du run. Les
exigences de secrets du workflow restent celles du dépôt ; un échec de ce job
doit être diagnostiqué depuis son journal, pas confondu avec le bug Home.
Vérifier le SHA du run : le simple nom « Nightly » ne prouve pas quel code tourne.
Aucun IPK ARM n'a été compilé localement sur l'hôte Linux x86_64.

Sur la TV, en conservant le même profil et plusieurs serveurs :

1. Installer l'IPK de cette branche, quitter complètement l'ancienne instance,
   puis ouvrir cette Nightly. Ne pas lancer de lecture ni modifier les éléments.
2. Laisser Home charger sans toucher la télécommande pendant environ 20 secondes.
   Photographier la rangée visible. Descendre sur Continue Watching avec la croix
   directionnelle (sans OK), puis photographier son premier affichage focalisé.
3. Si la rangée n'est pas au début, la ramener au début avec Gauche. Attendre
   encore 20 secondes et noter si elle reste en place. Capturer le journal avant
   de quitter : le lancement suivant peut réinitialiser le journal.
4. Quitter complètement puis rouvrir deux fois, et répéter les étapes 2–3.
   Conserver un journal distinct par lancement, y compris si le problème disparaît.
5. Si possible, refaire une fois avec un seul serveur épinglé à Home, puis avec
   les deux. Noter cette différence ; aucune modification d'historique Plex n'est
   nécessaire.

À transmettre : SHA/version du build, nombre de serveurs, photos avant/après,
ordre des gestes et journaux de chacun des lancements. Pour une installation
Nightly, le fichier est normalement :

```text
/tmp/com.beb.plxnative.nightly/plxnative-events.log
```

Avec les outils du dépôt et la TV déjà configurée, cette collecte est en lecture
seule, sans relancer l'application :

```sh
tools/tv-session.sh log --flavor nightly > cw-launch-1.log
```

Conserver notamment les lignes `install:`, `cwdiag:` et `hubs:`. Le premier
identifie l'installation. Les traces `lane` et `project offset` montrent la fenêtre
de données ; `project scroll`, `focus`, `motion` et `pan` montrent les déplacements
du viewport, leur provenance et les changements d'indice. Un `offset=0` avec un
scroll élevé pointera vers le chemin UI plutôt que vers le défaut de fenêtre.
