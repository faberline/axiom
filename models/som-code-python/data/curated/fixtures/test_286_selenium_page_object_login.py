import pytest

from candidate import DashboardPage, LoginError, LoginPage

BASE = "http://app.test"


class FakeElement:
    def __init__(self, app, name, text="", value=""):
        self.app = app
        self.name = name
        self.text = text
        self.value = value

    def clear(self):
        self.value = ""

    def send_keys(self, value):
        self.value += value

    def click(self):
        self.app.clicked(self.name)


class FakeApp:
    def __init__(self, users, prefill=""):
        self.users = users
        self.current_url = "about:blank"
        self.visited = []
        self.extra_username = False
        self.username = FakeElement(self, "username", value=prefill)
        self.password = FakeElement(self, "password")
        self.submit = FakeElement(self, "submit")
        self.flash = []
        self.logged_in = None

    def get(self, url):
        self.visited.append(url)
        self.current_url = url

    def find_elements(self, by, value):
        page = self.current_url
        if page.endswith("/login"):
            if (by, value) == ("id", "username"):
                return [self.username] * (2 if self.extra_username else 1)
            if (by, value) == ("id", "password"):
                return [self.password]
            if value == "button[type=submit]":
                return [self.submit]
            if value == ".flash.error":
                return self.flash
        if "/dashboard" in page:
            if value == "h1":
                return [FakeElement(self, "h1", text=f"  Hello, {self.logged_in}  ")]
            if (by, value) == ("link text", "Log out"):
                return [FakeElement(self, "logout")]
        return []

    def clicked(self, name):
        if name == "submit":
            if self.users.get(self.username.value) == self.password.value:
                self.logged_in = self.username.value
                self.current_url = BASE + "/dashboard?welcome=1"
            else:
                self.flash = [
                    FakeElement(self, "flash", text=" Invalid credentials \n")
                ]
        elif name == "logout":
            self.current_url = BASE + "/login"


def test_successful_login_reaches_dashboard():
    app = FakeApp({"ada": "pw"})
    page = LoginPage(app, BASE + "/").open()
    assert app.visited == [BASE + "/login"]
    assert page.is_current()
    dashboard = page.login("ada", "pw")
    assert isinstance(dashboard, DashboardPage)
    assert dashboard.greeting() == "Hello, ada"
    dashboard.logout()
    assert LoginPage(app, BASE).is_current()


def test_prefilled_fields_are_cleared_before_typing():
    app = FakeApp({"ada": "pw"}, prefill="guest")
    assert LoginPage(app, BASE).open().login("ada", "pw").greeting() == "Hello, ada"


def test_flash_error_message_is_raised():
    app = FakeApp({"ada": "pw"})
    page = LoginPage(app, BASE).open()
    with pytest.raises(LoginError, match="^Invalid credentials$"):
        page.login("ada", "nope")


def test_ambiguous_locator_is_rejected():
    app = FakeApp({"ada": "pw"})
    app.extra_username = True
    with pytest.raises(LoginError, match="found 2"):
        LoginPage(app, BASE).open().login("ada", "pw")


def test_blank_credentials_are_refused_before_typing():
    app = FakeApp({"ada": ""})
    page = LoginPage(app, BASE).open()
    with pytest.raises(LoginError, match="required"):
        page.login("ada", "")
    with pytest.raises(LoginError, match="required"):
        page.login("", "pw")
    assert app.username.value == ""
    assert app.logged_in is None
