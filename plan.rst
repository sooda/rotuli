programming plan
================

Still not a single line of actual program code, but here's what it should do.


actions
-------

Most of the outer loops can be parallelized.

always: (run this in threads)

- foreach source file
    * read it
    * parse metadata
    * parse markup
    * set up local things, like destination url, perhaps render markup already here

output generation:

- fold source files into lists of groups (perhaps define groups as metadata keys? why not?), merge to one list
- find out every page in every group, sort groups by group value (or by page date or something?) per page
- fold source files into original-translated links (original doesn't know it's translated yet)
- assign original-translated lists to their link nodes
- foreach source file
    * set up template context from metadata and core stuff
    * build output file using template engine

thumbnail mangling:

- foreach source file:
    * render, verify existence or however mangle image thumbnails


datatypes
---------


site
~~~~

some group of pages
also groups, although they aren't directly rendered


page
~~~~

* original metadata from source file
* content
* disk url
* http url
* group pointers or names
* translation page pointers (just a group?)
* assets (images) referenced in content?


metadata
~~~~~~~~

* queryable yaml key/pair with appropriate datatypes (e.g., list or date), impl hidden somehow


content
~~~~~~~

* renderable rst data, impl hidden somehow


template
~~~~~~~~

* a box that eats pages and metadata and produces rendered data


group
~~~~~

* name
* list of page pointers
* name is the url slug, title etc by page metadata when each group has a matching page (e.g., categories)
