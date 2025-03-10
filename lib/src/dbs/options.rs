use super::capabilities::Capabilities;
use crate::cnf;
use crate::dbs::Notification;
use crate::err::Error;
use crate::iam::{Action, Auth, ResourceKind, Role};
use crate::sql::Base;
use channel::Sender;
use std::sync::Arc;
use uuid::Uuid;

/// An Options is passed around when processing a set of query
/// statements. An Options contains specific information for how
/// to process each particular statement, including the record
/// version to retrieve, whether futures should be processed, and
/// whether field/event/table queries should be processed (useful
/// when importing data, where these queries might fail).
#[derive(Clone, Debug)]
pub struct Options {
	/// Current Node ID
	id: Option<Uuid>,
	/// Currently selected NS
	ns: Option<Arc<str>>,
	/// Currently selected DB
	db: Option<Arc<str>>,
	/// Approximately how large is the current call stack?
	dive: u8,
	/// Connection authentication data
	pub auth: Arc<Auth>,
	/// Is authentication enabled?
	pub auth_enabled: bool,
	/// Whether live queries are allowed?
	pub live: bool,
	/// Should we force tables/events to re-run?
	pub force: bool,
	/// Should we run permissions checks?
	pub perms: bool,
	/// Should we error if tables don't exist?
	pub strict: bool,
	/// Should we process field queries?
	pub fields: bool,
	/// Should we process event queries?
	pub events: bool,
	/// Should we process table queries?
	pub tables: bool,
	/// Should we process index queries?
	pub indexes: bool,
	/// Should we process function futures?
	pub futures: bool,
	/// Should we process variable field projections?
	pub projections: bool,
	/// The channel over which we send notifications
	pub sender: Option<Sender<Notification>>,
	/// Datastore capabilities
	pub capabilities: Arc<Capabilities>,
}

impl Default for Options {
	fn default() -> Self {
		Options::new()
	}
}

impl Options {
	/// Create a new Options object
	pub fn new() -> Options {
		Options {
			id: None,
			ns: None,
			db: None,
			dive: 0,
			live: false,
			perms: true,
			force: false,
			strict: false,
			fields: true,
			events: true,
			tables: true,
			indexes: true,
			futures: false,
			projections: false,
			auth_enabled: true,
			sender: None,
			auth: Arc::new(Auth::default()),
			capabilities: Arc::new(Capabilities::default()),
		}
	}

	// --------------------------------------------------

	/// Specify which Namespace should be used for
	/// code which uses this `Options` object.
	pub fn set_ns(&mut self, ns: Option<Arc<str>>) {
		self.ns = ns
	}

	/// Specify which Database should be used for
	/// code which uses this `Options` object.
	pub fn set_db(&mut self, db: Option<Arc<str>>) {
		self.db = db
	}

	// --------------------------------------------------

	/// Set all the required options from a single point.
	/// The system expects these values to always be set, so this should be called for all
	/// instances when there is doubt.
	pub fn with_required(
		mut self,
		node_id: uuid::Uuid,
		ns: Option<Arc<str>>,
		db: Option<Arc<str>>,
		auth: Arc<Auth>,
	) -> Self {
		self.id = Some(node_id);
		self.ns = ns;
		self.db = db;
		self.auth = auth;
		self
	}

	/// Set the Node ID for subsequent code which uses
	/// this `Options`, with support for chaining.
	pub fn with_id(mut self, id: Uuid) -> Self {
		self.id = Some(id);
		self
	}

	/// Specify which Namespace should be used for code which
	/// uses this `Options`, with support for chaining.
	pub fn with_ns(mut self, ns: Option<Arc<str>>) -> Self {
		self.ns = ns;
		self
	}

	/// Specify which Database should be used for code which
	/// uses this `Options`, with support for chaining.
	pub fn with_db(mut self, db: Option<Arc<str>>) -> Self {
		self.db = db;
		self
	}

	/// Specify the authentication options for subsequent
	/// code which uses this `Options`, with chaining.
	pub fn with_auth(mut self, auth: Arc<Auth>) -> Self {
		self.auth = auth;
		self
	}

	/// Specify whether live queries are supported for
	/// code which uses this `Options`, with chaining.
	pub fn with_live(mut self, live: bool) -> Self {
		self.live = live;
		self
	}

	/// Specify whether permissions should be run for
	/// code which uses this `Options`, with chaining.
	pub fn with_perms(mut self, perms: bool) -> Self {
		self.perms = perms;
		self
	}

	///
	pub fn with_force(mut self, force: bool) -> Self {
		self.force = force;
		self
	}

	///
	pub fn with_strict(mut self, strict: bool) -> Self {
		self.strict = strict;
		self
	}

	///
	pub fn with_fields(mut self, fields: bool) -> Self {
		self.fields = fields;
		self
	}

	///
	pub fn with_events(mut self, events: bool) -> Self {
		self.events = events;
		self
	}

	///
	pub fn with_tables(mut self, tables: bool) -> Self {
		self.tables = tables;
		self
	}

	///
	pub fn with_indexes(mut self, indexes: bool) -> Self {
		self.indexes = indexes;
		self
	}

	///
	pub fn with_futures(mut self, futures: bool) -> Self {
		self.futures = futures;
		self
	}

	///
	pub fn with_projections(mut self, projections: bool) -> Self {
		self.projections = projections;
		self
	}

	/// Create a new Options object for a subquery
	pub fn with_import(mut self, import: bool) -> Self {
		self.fields = !import;
		self.events = !import;
		self.tables = !import;
		self
	}

	/// Create a new Options object with auth enabled
	pub fn with_auth_enabled(mut self, auth_enabled: bool) -> Self {
		self.auth_enabled = auth_enabled;
		self
	}

	/// Create a new Options object with the given Capabilities
	pub fn with_capabilities(mut self, capabilities: Arc<Capabilities>) -> Self {
		self.capabilities = capabilities;
		self
	}

	// --------------------------------------------------

	/// Create a new Options object for a subquery
	pub fn new_with_perms(&self, perms: bool) -> Self {
		Self {
			sender: self.sender.clone(),
			auth: self.auth.clone(),
			capabilities: self.capabilities.clone(),
			ns: self.ns.clone(),
			db: self.db.clone(),
			perms,
			..*self
		}
	}

	/// Create a new Options object for a subquery
	pub fn new_with_force(&self, force: bool) -> Self {
		Self {
			sender: self.sender.clone(),
			auth: self.auth.clone(),
			capabilities: self.capabilities.clone(),
			ns: self.ns.clone(),
			db: self.db.clone(),
			force,
			..*self
		}
	}

	/// Create a new Options object for a subquery
	pub fn new_with_strict(&self, strict: bool) -> Self {
		Self {
			sender: self.sender.clone(),
			auth: self.auth.clone(),
			capabilities: self.capabilities.clone(),
			ns: self.ns.clone(),
			db: self.db.clone(),
			strict,
			..*self
		}
	}

	/// Create a new Options object for a subquery
	pub fn new_with_fields(&self, fields: bool) -> Self {
		Self {
			sender: self.sender.clone(),
			auth: self.auth.clone(),
			capabilities: self.capabilities.clone(),
			ns: self.ns.clone(),
			db: self.db.clone(),
			fields,
			..*self
		}
	}

	/// Create a new Options object for a subquery
	pub fn new_with_events(&self, events: bool) -> Self {
		Self {
			sender: self.sender.clone(),
			auth: self.auth.clone(),
			capabilities: self.capabilities.clone(),
			ns: self.ns.clone(),
			db: self.db.clone(),
			events,
			..*self
		}
	}

	/// Create a new Options object for a subquery
	pub fn new_with_tables(&self, tables: bool) -> Self {
		Self {
			sender: self.sender.clone(),
			auth: self.auth.clone(),
			capabilities: self.capabilities.clone(),
			ns: self.ns.clone(),
			db: self.db.clone(),
			tables,
			..*self
		}
	}

	/// Create a new Options object for a subquery
	pub fn new_with_indexes(&self, indexes: bool) -> Self {
		Self {
			sender: self.sender.clone(),
			auth: self.auth.clone(),
			capabilities: self.capabilities.clone(),
			ns: self.ns.clone(),
			db: self.db.clone(),
			indexes,
			..*self
		}
	}

	/// Create a new Options object for a subquery
	pub fn new_with_futures(&self, futures: bool) -> Self {
		Self {
			sender: self.sender.clone(),
			auth: self.auth.clone(),
			capabilities: self.capabilities.clone(),
			ns: self.ns.clone(),
			db: self.db.clone(),
			futures,
			..*self
		}
	}

	/// Create a new Options object for a subquery
	pub fn new_with_projections(&self, projections: bool) -> Self {
		Self {
			sender: self.sender.clone(),
			auth: self.auth.clone(),
			capabilities: self.capabilities.clone(),
			ns: self.ns.clone(),
			db: self.db.clone(),
			projections,
			..*self
		}
	}

	/// Create a new Options object for a subquery
	pub fn new_with_import(&self, import: bool) -> Self {
		Self {
			sender: self.sender.clone(),
			auth: self.auth.clone(),
			capabilities: self.capabilities.clone(),
			ns: self.ns.clone(),
			db: self.db.clone(),
			fields: !import,
			events: !import,
			tables: !import,
			..*self
		}
	}

	/// Create a new Options object for a subquery
	pub fn new_with_sender(&self, sender: Sender<Notification>) -> Self {
		Self {
			auth: self.auth.clone(),
			capabilities: self.capabilities.clone(),
			ns: self.ns.clone(),
			db: self.db.clone(),
			sender: Some(sender),
			..*self
		}
	}

	// Get currently selected base
	pub fn selected_base(&self) -> Result<Base, Error> {
		match (self.ns.as_ref(), self.db.as_ref()) {
			(None, None) => Ok(Base::Root),
			(Some(_), None) => Ok(Base::Ns),
			(Some(_), Some(_)) => Ok(Base::Db),
			(None, Some(_)) => Err(Error::NsEmpty),
		}
	}

	/// Create a new Options object for a function/subquery/future/etc.
	///
	/// The parameter is the approximate cost of the operation (more concretely, the size of the
	/// stack frame it uses relative to a simple function call). When in doubt, use a value of 1.
	pub fn dive(&self, cost: u8) -> Result<Self, Error> {
		let dive = self.dive.saturating_add(cost);
		if dive <= *cnf::MAX_COMPUTATION_DEPTH {
			Ok(Self {
				sender: self.sender.clone(),
				auth: self.auth.clone(),
				capabilities: self.capabilities.clone(),
				ns: self.ns.clone(),
				db: self.db.clone(),
				dive,
				..*self
			})
		} else {
			Err(Error::ComputationDepthExceeded)
		}
	}

	// --------------------------------------------------

	/// Get current Node ID
	pub fn id(&self) -> Result<Uuid, Error> {
		self.id.ok_or(Error::Unreachable)
	}

	/// Get currently selected NS
	pub fn ns(&self) -> &str {
		self.ns.as_ref().map(AsRef::as_ref).unwrap()
		// self.ns.as_ref().map(AsRef::as_ref).ok_or(Error::Unreachable)
	}

	/// Get currently selected DB
	pub fn db(&self) -> &str {
		self.db.as_ref().map(AsRef::as_ref).unwrap()
		// self.db.as_ref().map(AsRef::as_ref).ok_or(Error::Unreachable)
	}

	/// Check whether this request supports realtime queries
	pub fn realtime(&self) -> Result<(), Error> {
		if !self.live {
			return Err(Error::RealtimeDisabled);
		}
		Ok(())
	}

	// Validate Options for Namespace
	pub fn valid_for_ns(&self) -> Result<(), Error> {
		if self.ns.is_none() {
			return Err(Error::NsEmpty);
		}
		Ok(())
	}

	// Validate Options for Database
	pub fn valid_for_db(&self) -> Result<(), Error> {
		self.valid_for_ns()?;

		if self.db.is_none() {
			return Err(Error::DbEmpty);
		}
		Ok(())
	}

	/// Check if the current auth is allowed to perform an action on a given resource
	pub fn is_allowed(&self, action: Action, res: ResourceKind, base: &Base) -> Result<(), Error> {
		// If auth is disabled, allow all actions for anonymous users
		if !self.auth_enabled && self.auth.is_anon() {
			return Ok(());
		}

		let res = match base {
			Base::Root => res.on_root(),
			Base::Ns => {
				self.valid_for_ns()?;
				res.on_ns(self.ns())
			}
			Base::Db => {
				self.valid_for_db()?;
				res.on_db(self.ns(), self.db())
			}
			Base::Sc(sc) => {
				self.valid_for_db()?;
				res.on_scope(self.ns(), self.db(), sc)
			}
		};

		self.auth.is_allowed(action, &res).map_err(Error::IamError)
	}

	/// Whether or not to check table permissions
	///
	/// TODO: This method is called a lot during data operations, so we decided to bypass the system's authorization mechanism.
	/// This is a temporary solution, until we optimize the new authorization system.
	pub fn check_perms(&self, action: Action) -> bool {
		// If permissions are disabled, don't check permissions
		if !self.perms {
			return false;
		}

		// If auth is disabled and actor is anonymous, don't check permissions
		if !self.auth_enabled && self.auth.is_anon() {
			return false;
		}

		// Is the actor allowed to view?
		let can_view =
			[Role::Viewer, Role::Editor, Role::Owner].iter().any(|r| self.auth.has_role(r));
		// Is the actor allowed to edit?
		let can_edit = [Role::Editor, Role::Owner].iter().any(|r| self.auth.has_role(r));
		// Is the target database in the actor's level?
		let db_in_actor_level = self.auth.is_root()
			|| self.auth.is_ns() && self.auth.level().ns().unwrap() == self.ns()
			|| self.auth.is_db()
				&& self.auth.level().ns().unwrap() == self.ns()
				&& self.auth.level().db().unwrap() == self.db();

		// Is the actor allowed to do the action on the selected database?
		let is_allowed = match action {
			Action::View => {
				// Today all users have at least View permissions, so if the target database belongs to the user's level, don't check permissions
				can_view && db_in_actor_level
			}
			Action::Edit => {
				// Editor and Owner roles are allowed to edit, but only if the target database belongs to the user's level
				can_edit && db_in_actor_level
			}
		};

		// Check permissions if the autor is not already allowed to do the action
		!is_allowed
	}
}
