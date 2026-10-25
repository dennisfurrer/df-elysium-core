//! Where the page is: the home screen of cards, or one tool, named in the URL's hash (`#/send`) so
//! the back button and links work.

/// A screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    Home,
    Import,
    Activity,
    Send,
    Token,
    Deposit,
    Withdraw,
    Claims,
    Ticket,
    Tickets,
    Deploy,
    Verify,
    Chain,
    Inspect,
    Live,
}

impl Route {
    pub const ALL: [Route; 15] = [
        Route::Home,
        Route::Import,
        Route::Activity,
        Route::Send,
        Route::Token,
        Route::Deposit,
        Route::Withdraw,
        Route::Claims,
        Route::Ticket,
        Route::Tickets,
        Route::Deploy,
        Route::Verify,
        Route::Chain,
        Route::Inspect,
        Route::Live,
    ];

    pub const fn path(self) -> &'static str {
        match self {
            Route::Home => "",
            Route::Import => "import",
            Route::Activity => "activity",
            Route::Send => "send",
            Route::Token => "erc20",
            Route::Deposit => "deposit",
            Route::Withdraw => "withdraw",
            Route::Claims => "claims",
            Route::Ticket => "ticket",
            Route::Tickets => "tickets",
            Route::Deploy => "deploy",
            Route::Verify => "verify",
            Route::Chain => "chain",
            Route::Inspect => "inspect",
            Route::Live => "live",
        }
    }

    pub fn href(self) -> String {
        format!("#/{}", self.path())
    }

    /// The screen a hash names; home for any other.
    pub fn from_hash(hash: &str) -> Route {
        let path = hash.trim_start_matches('#').trim_start_matches('/');
        Route::ALL.into_iter().find(|route| route.path() == path).unwrap_or(Route::Home)
    }

    /// The screen the URL names now.
    pub fn current() -> Route {
        let hash = web_sys::window().and_then(|window| window.location().hash().ok());
        hash.map_or(Route::Home, |hash| Route::from_hash(&hash))
    }
}

/// Goes to `route`, as following its link would.
pub fn go(route: Route) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(&route.href());
    }
}
