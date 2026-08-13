# GTK4

## Dropdown

- A dropdown list is composed from a GListModel. 
- the GtkListItemFactory determines, how the elements in the list model are displayed
- a GListModel represents a list of mutable GObjects. A list may for example be used to display the currently visible WiFi networks nearby
- the GListModel has the signal `items-changed`, which signals that the members of the list changed
- the GListModel only reports changes to the list membership, not changes to the individual items
- `get_n_items` may be used to retrieve the number of items in the list
- `get_item` may be used to retrieve an element from the list


- ein Dropdown wird aus einem GListModel gemacht. GListModel ist selbst kein Typ sondern nur ein Interface, das bspw. durch ListStore umgesetzt wird, aber auch durch StringList. 
- ListStore erlaubt das speichern von beliebigen Inhalten im Speicher
- ListSotre: https://docs.gtk.org/gio/class.ListStore.html

Plan: ListStore erstellen, wo (ID, Name) Paare von Pipewire IDs un App Namen drin stehen