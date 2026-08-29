- [x] new block starts
  - [x] setext heading
  - [x] thematic break
  - [x] new list item continuing
  - [x] quotes
  - [x] new list
  - [x] fenced code blocks
  - [x] atx_heading
  - [x] lazy paragraph continuation
  - [x] reference link defs
- [x] incorporate tabs
  - [x] fix space counting code
    - [x] atx_heading
- [x] implement correct new line code
- [x] unicode property macro
- [x] create scripts for setting up the repo from a fresh pull
  - [x] curl
    - [x] spec
    - [x] unicode 
    - [x] html characters
- [x] create build scripts for unicode properties and json_tests 
- [x] inline parsing
  - [x] code spans
  - [x] emphasis.
  - [x] links
  - [x] Images
  - [x] Autolinks
  - [x] html
  - [x] soft line break
  - [x] hard line breaks
- [x] text stuff
  - [x] entity and numeric char references.
  - [x] backslash escapes.
  - [x] insecure characters

- [x] refactor char index iterators to only return char (no index)
- [x] refactor close_paragraph
- [x] refactor html block things
- [x] refactor create new blocks
- [x] bs escaping in link dests.
- [x] disable prev brackets from links.
- [x] images
- [x] better char writing ergonomics.


- [ ] ACTUALLY MAKE A FREAKING WEBPAGE WITH THIS PARSER I MADE!!!

Improvements/performance optimization:
- [ ] make Block use an Arena! 
- [ ] make all inlines/code blocks use the same stuff.
- [ ] put blocks in Document inside of refcounts. This should allow multiple access to 
      it without having to "climb" down the tree every time.
- [ ] type parameterize blocks, First they fill up with raw strings, then we can transform them to blocks with parsed inlines.
- [ ] No longer copy string when we create new Paragraphs/headings. Just have the entire document be
      one string and the Inline contain a vector of &str s pointing inside of it
      - [ ] requires new iterator over the inline string. it will be based on a Vec<&str> instead of a String. 
      - [ ] create the illusion of one continuous string by returning '\n' when an interior &str iterator reaches "end of line"
- [ ] neaten up the DLL implementation. 
      - [ ] use Options/Refcounts for the nodes in the list.
      - [ ] make the InlineTextComponent take ownership when converting.
