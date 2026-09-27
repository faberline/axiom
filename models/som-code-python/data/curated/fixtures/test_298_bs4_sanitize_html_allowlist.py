from candidate import sanitize


def test_allowed_markup_is_kept():
    assert sanitize("<p>Hi <b>there</b><br/></p>") == "<p>Hi <b>there</b><br/></p>"


def test_unknown_tags_are_unwrapped_but_text_kept():
    assert sanitize("<div><span>keep</span> me</div>") == "keep me"


def test_dangerous_tags_are_dropped_with_content():
    out = sanitize("<p>a<script>alert(1)</script><style>p{}</style>b</p>")
    assert out == "<p>ab</p>"


def test_attributes_are_filtered():
    out = sanitize(
        '<p onclick="x()" class="c">t</p><a href="/x" title="T" id="i">l</a>'
    )
    assert out == '<p>t</p><a href="/x" rel="nofollow noopener" title="T">l</a>'


def test_unsafe_urls_are_removed():
    out = sanitize(
        '<a href=" JavaScript:alert(1)">x</a><a href="data:text/html,1">y</a>'
    )
    assert out == "<a>x</a><a>y</a>"
    assert 'href="mailto:a@b.c"' in sanitize('<a href="mailto:a@b.c">m</a>')


def test_comments_are_removed():
    assert sanitize("<p>a<!-- secret -->b</p>") == "<p>ab</p>"


def test_nested_dangerous_tag_inside_unknown_tag():
    assert sanitize("<div>x<iframe><b>y</b></iframe>z</div>") == "xz"
