templates
=========

Rotulus expects you to implement site-specific logic using templates only.
A number of context variables and functions are provided.


metadata
--------

Some of the metadata provided by you for every page is special:

* template: the template used for rendering this page, read from the "templates" directory. If this key doesn't exist, "default.html" is used.
* original: the url of this page in its original language, when this page is a translation in another language. Either absolute if begins with a slash or relative. TODO source or dest path?
* sort: if provided, marks by name the metadata key to be used to sort this group by default (XXX for what group? and in every post? nope, just the first one?)
* no_directory: don't make a directory for this page with the auto-index.html feature. just strip the markup extension and generate in-place url (e.g., rss.xml.rst)
* title: overrides one found in the content markup, if it has one (XXX not necessary?)

Anything else is passed to the template in the context with no special handling.
The special ones are included, too.


context
-------

The template context is page-specific; there is the metadata provided by you, and some "magic" variables that are different for each page.

All metadata specified in the page is visible as attributes in the "meta" object. (XXX or just flatten out to globals?)
Additionally, the actual content of a page is special, and exits as "content", with also "content_preview" containing the first paragraph.
If you want to write a short preview yourself and use that in templates, it's fine too; use a string in the metadata for that.
The document content is also parsed for the top-level heading, which exists as "title".
Finally, Rotulus provides some core variables gluing the site together; these are found in the global namespace.

All of the pages are magically linked together: in each template you have access to all groups of pages, so that pagination, listing, and preview displays, etc. can be made to work.

* url: the location of the page, as visible to the browser. ``you are <a href="{{ url }}">here</a>``
* path: the location of the page, as parsed from the filesystem. ``source: github.com/yourname/blog/{{ path }}``
* languages (XXX translations?): a dictionary of all versions of this page, keyed by the language. ``in english: <a href="{{ languages.en.url }}">``
* groups: a dict of page lists that belong in a particular group specified in the metadata. XXX all keys in metadata specify a group, to make things simple? Then also drafts etc become groups, and: ``for i in groups.posts``
* group: has pages as "previous", "current", "next" attributes for each group. ``previous page: <a href="{{ group.posts.previous.url }}">`` XXX merge with groups?
* pager: pagination support, borrowed from django; don't confuse these pages with the web pages.


context variable types
~~~~~~~~~~~~~~~~~~~~~~

Other than plain strings or lists or the like:

* page: has the whole template context as attributes like in the global namespace while rendering one; is contained in groups.NAME[i], and groups.NAME.previous, for example.
* group: previous, current, next
* pager: previous, next, current, range (numbers). The previous and next attributes exist only if this is not the first or the last page, respectively.

Metadata types are represented in the best way possible:

* strings work as strings
* lists can be indexed and looped
* date/time types have .year, .hour etc.
