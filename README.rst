rotulus static site generator
=============================

(i'd prefer the word compiler instead of generator but generator seems to be the globally accepted one.)

Rotulus is a roll that holds some papyrus that holds writing.
Rotulus is dumb, it just is a support structure for your data, making it slightly easier to present it.
You just format the data with your templates.
To make that possible, everything necessary is passed as a context in to the templates.

Rotulus is generic; no site-specific modifications in its code should be required.

Supports reStructuredText markup for the source data, and Tera (Jinja2/Django style) templates.
If I change my mind, those are supposed to be easy enough to change.
No template code is allowed in the source data, because data should be tool-agnostic.
All templates are yours; rotulus does not provide any.

The core does not support much else than:

* rendering the data through templates
* arbitrary metadata embedded in the source data, passed to the templates
* collections of pages, so that templates can list all blog posts, categories, and such
* drafts, not included in the output unless you say so (implemented by filtering by a metadata boolean)
* linking between the same page written in different languages
* scaling large source images for thumbs and links to originals
* semi-automatic pagination

Written in rust because i need to learn it.
Not meant to be used by anyone else, just my personal toy.
