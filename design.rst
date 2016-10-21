Early design plans
==================

I looked at several simple generators (i don't want a wordpress-sized thing) and most were too large, some were okay, some too hardcoded for, e.g., very post-centric approach, and I just wanted to learn me some Rust.
Previously, I was planning to write this in Python, or to find one made in Python so I'd know how to hack it.
I guess Jekyll would be good enough, too, just quickly skimmed it.

Here's some mixed things I want for the new blog and for the engine.


good looking samples
--------------------

- Nice layout and good source structure http://lucumr.pocoo.org https://github.com/mitsuhiko/lucumr https://github.com/mitsuhiko/rstblog/tree/master/rstblog
- http://stevelosh.com
- per-post images, and i like the colors here http://www.windytan.com/ (todo: diy background with random halton dots or something? gvim-ish colorscheme?)
- http://gitready.com
- http://ithare.com/
- https://fgiesen.wordpress.com, not so minimal tho but the small number of categories for the navi bar is nice
- some medium.com blogs
- colorscheme http://featherweightmusings.blogspot.fi/ but i won't use separate backgrounds like that
- https://hoverbear.org


high level requirements
-----------------------

- read plaintext git, no web ui bullshit
- git post-commit hook to render stuff to html dir
- really generic, not so blog post centric although most content will be blogging
- understand lists of things: categories, all posts, posts of a certain date range, posts in a certain category
  - those generic "lists of things" for "for p in posts" or "in whatever" specified in site contents, not in engine
- simple core and some module system to incrementally add things like image thumbnailer, an embedded video player, a source code highlighter or page comments
- templates, not displayable as-is but used for compiling content into html, sort of like previously and in a typical dynamic website
- things like the markup language or the template engine somewhat pluggable if i change my opinion on those, nothing designed around specifics of one single implementation
- no templates, static stuff or anything in engine but all content in site repo
- some content written in a markup language, rendered to html
- some static content which does not get modified during compilation, just copied (need magic for thumbnails)
- same content in multiple languages, linked together
- draft pages displayed only if whole site compiled as draft
- ability to implement an rss feed easily
- pagination
- prev-next links in posts


random details
--------------

- some "front matter" yaml metadata on each content source to specify layout type (blog/category/...), bools like published/draft, categories this belongs in, deprecated tags from old site, post date etc.
- also maybe a global metadata file to contain things like page source links to github or whatever unless possible to trick with templates only?
- link to source repo in each compiled page
- pagination starts from the earliest page so urls don't change, most recent page number is the largest
  - dynamically generated pages, like with pagination, either need a config file or metadata in the page that gets paginated
  - metadata in index: pass the start index to the template, specify pagination url in front matter
  - alternatively in page.html (or similar), not the front welcome page anymore, i guess this is nice and explicit
- whole site read into memory first and parsed to get complete groups, then able to render those as listings by date/tag/category/language/...
- blog categories in separate files, rendered to listings with description of the category
- no template tags in content at all to keep things orthogonal, whole source is just data compiled into a single content block in the template
  - source data doesn't know which template system is used
  - this means, e.g., no magic template tags for images, internal links, or anything
  - slightly worse to write, more separated so that the source can be rendered elsewhere too without the engine
  - make the engine see things like images in the markup and build thumbs for them to ease with the layout


site layout
-----------

- trivial multilingual support by unique urls; link translations to original url in source metadata
- / front page like previously
- /pages/N/ display latest posts paginated, like previously with ?page=x
- /yyyy/mm/dd/post/ like now, but without global sidebar bullshit
- /category/name/, /category/name/N/ to describe this category and list all posts, paginated
- /category/ to list all categories
- /archive/ just as the old sidebar, plain year/month list of posts
- /about/ and other ordinary non-post pages visible as-is with no other magic
- each shortened blog (first paragraph) in listings to contain a selected picture if available
- maybe display a separate lead paragraph instead of just the first paragraph in lists of posts


content type
------------

- optional extra data for template context in source metadata, if something else than title and content needed (title parsed from the first markup header probably)
- super simple to write so it's easy to just write the damn blog
- per-blogentry media, or do i want a global arbitrary per-site media storage with some magic to include any of those too?
- template metadata substitutes title if none available in the content for nearly empty pages that have most of their content in templates (like a blog archive)


content hierarchy
-----------------

- source/ contains the to-be-compiled content
- source/whatever/path/here/index.rst -> compiled to .../index.html
- source/whatever/path/here/thing.rst -> compiled to .../thing/index.html
- source/foo.{anything else than supported markup} is an error
- static/path/here/any.thing copied as-is, simply a cp -r (TODO thumbnails? separate tool to generate? not in source repo)
- templates/ for those non-displayable things used for rendering


output generation
-----------------

- nice templates
- no minification or other tricks, not so much traffic and could use http compression i guess
- no integrated httpd for debug bullshit, i have proper server software for this


template engine
---------------

- i like the extends block inheritance thing in ninja2
- filter support required, custom filters
- also custom tags that the engine doesn't supply on its own (or then just use my own fork)

- liquid doesn't have inheritance?
  http://www.sameratiani.com/2011/10/22/get-jekyll-working-with-liquid-inheritance.html
  https://github.com/cobalt-org/liquid-rust

- tera seems pretty good for this
  https://blog.wearewizards.io/introducing-tera-a-template-engine-in-rust
  got filters just recently? https://github.com/Keats/tera/commit/7a68a1e4125dce4ec9978fafdd9bbbadc9249ea5
  - "Tera will panic on invalid templates which means you should add template compilation as a build step when compiling" -- i wouldn't want to rustc when editing templates (creates dependency between compiler and site) but maybe can live with this
  - for-else support?


publishing
----------

- a simple command line batch tool, preferably as a git commit hook
- set up production and draft sites separately, preview drafts easily
- whole site to update at once and git commit id in output to be explicit about what is published, ln -s $gitcommit tmpname; mv -f tmpname public-thing
- incremental updates: detect changes in src, render only what is necessary (changed pages, their reverse deps), use hard links in filesystem
- generate thumbnails of the images displayed anywhere (detecting their inline size, if specified). this needs to be cached because i have a zillion of those, maybe run as a separate step or copy as hard links and rsync trickery
