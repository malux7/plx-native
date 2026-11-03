# Last selected library after restarting the app

The reported behaviour was asymmetric: Movies reopened on the first library after
a full restart, while Shows restored the last selected library. Both kept a newly
selected library while the app remained running. The selected libraries were
confirmed to be favourites.

The saved preference was not missing. `DirectoryView::kind_address(Movie)` already
identified the saved library, but `Bridge::aim_library` skipped it whenever the
store's current section had the requested kind. At startup that section can simply
be the first discovered Movies library. The Library screen also accepts a current
section of the requested kind, so it did not repair the selection after mounting.
Shows worked because moving from the initial Movie section changed kind and took
the preferred-library path.

The fix restores the preferred library before registering the first library intent.
After an intent exists, the existing same-kind rule keeps the current selection,
including a non-favourite library deliberately visited in the running app, rather
than overriding it with an older stored preference. The existing
`AppMounter::library_kind` carries that intent; no new preference or parallel
selection registry is introduced. Browse reset clears it and the queued Library
commands so a retained Bridge does not reuse another profile's navigation intent.

The regression uses a temporary stored session with remembered Movie and Show
libraries, then creates a fresh Browse owner pointing at the first Movie library.
It calls the real `nav_tab` route. Before the fix Movies stayed on section 1 rather
than the remembered section 3, while Shows restored its remembered section 4.
The tests also cover subsequent same-kind visits, a current choice differing from
the stored snapshot, a non-favourite visit, and a session/table reset.

TV validation is still required: with both libraries marked as favourites, select
a non-first Movies library, fully close and reopen the Nightly, wait for Home to
finish loading, then open Movies. Repeat for Shows and confirm that changing the
selection during the running session still keeps that choice. A second pass can
open Movies immediately after launch to distinguish the fully discovered table
from a late-arriving saved library.
