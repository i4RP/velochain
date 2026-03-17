//! Extended UI panels for crafting, quest log, skill tree, and shop.
//!
//! Provides toggleable overlay panels that integrate with the
//! game-engine's crafting, quest, skill, and shop systems.
//! Each panel is shown/hidden via keyboard shortcut or button.

use bevy::prelude::*;

/// Plugin for extended UI panels.
pub struct UiPanelsPlugin;

impl Plugin for UiPanelsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PanelStates::default())
            .insert_resource(NotificationQueue::default())
            .add_event::<PanelEvent>()
            .add_systems(Startup, setup_panels)
            .add_systems(Update, (
                handle_panel_events,
                handle_panel_hotkeys,
                update_notifications,
            ));
    }
}

// ---------------------------------------------------------------------------
// Panel state management
// ---------------------------------------------------------------------------

/// Which panels are currently visible.
#[derive(Resource, Default)]
pub struct PanelStates {
    pub crafting_open: bool,
    pub quest_log_open: bool,
    pub skill_tree_open: bool,
    pub shop_open: bool,
    /// Which panel was most recently opened (for focus/z-order).
    pub active_panel: Option<PanelKind>,
}

impl PanelStates {
    /// Close all panels.
    pub fn close_all(&mut self) {
        self.crafting_open = false;
        self.quest_log_open = false;
        self.skill_tree_open = false;
        self.shop_open = false;
        self.active_panel = None;
    }

    /// Toggle a specific panel, closing others.
    pub fn toggle(&mut self, kind: PanelKind) {
        let was_open = self.is_open(kind);
        self.close_all();
        if !was_open {
            self.set_open(kind, true);
            self.active_panel = Some(kind);
        }
    }

    /// Check if a panel is open.
    pub fn is_open(&self, kind: PanelKind) -> bool {
        match kind {
            PanelKind::Crafting => self.crafting_open,
            PanelKind::QuestLog => self.quest_log_open,
            PanelKind::SkillTree => self.skill_tree_open,
            PanelKind::Shop => self.shop_open,
        }
    }

    fn set_open(&mut self, kind: PanelKind, open: bool) {
        match kind {
            PanelKind::Crafting => self.crafting_open = open,
            PanelKind::QuestLog => self.quest_log_open = open,
            PanelKind::SkillTree => self.skill_tree_open = open,
            PanelKind::Shop => self.shop_open = open,
        }
    }

    /// Count how many panels are open.
    pub fn open_count(&self) -> usize {
        [self.crafting_open, self.quest_log_open, self.skill_tree_open, self.shop_open]
            .iter()
            .filter(|&&v| v)
            .count()
    }
}

/// Panel identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelKind {
    Crafting,
    QuestLog,
    SkillTree,
    Shop,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Panel interaction events.
#[derive(Event, Clone, Debug)]
pub enum PanelEvent {
    /// Toggle a panel open/closed.
    Toggle(PanelKind),
    /// Close all panels.
    CloseAll,
    /// Crafting: select a recipe.
    SelectRecipe { recipe_id: u32 },
    /// Crafting: attempt to craft.
    CraftItem { recipe_id: u32 },
    /// Quest: select a quest for details.
    SelectQuest { quest_id: String },
    /// Quest: abandon a quest.
    AbandonQuest { quest_id: String },
    /// Skill: allocate a skill point.
    AllocateSkill { skill_id: String },
    /// Shop: buy an item.
    ShopBuy { item_id: u32, quantity: u32 },
    /// Shop: sell an item.
    ShopSell { item_id: u32, quantity: u32 },
}

// ---------------------------------------------------------------------------
// Notification system
// ---------------------------------------------------------------------------

/// Toast notification queue for showing brief messages.
#[derive(Resource)]
pub struct NotificationQueue {
    /// Active notifications.
    pub notifications: Vec<Notification>,
    /// Maximum visible notifications.
    pub max_visible: usize,
    /// Default display duration.
    pub default_duration: f32,
}

impl Default for NotificationQueue {
    fn default() -> Self {
        Self {
            notifications: Vec::new(),
            max_visible: 5,
            default_duration: 3.0,
        }
    }
}

impl NotificationQueue {
    /// Push a new notification.
    pub fn push(&mut self, text: String, kind: NotificationKind) {
        let duration = self.default_duration;
        self.notifications.push(Notification {
            text,
            kind,
            remaining: duration,
            total: duration,
        });

        // Trim old notifications if over limit
        while self.notifications.len() > self.max_visible * 2 {
            self.notifications.remove(0);
        }
    }

    /// Push with custom duration.
    pub fn push_timed(&mut self, text: String, kind: NotificationKind, duration: f32) {
        self.notifications.push(Notification {
            text,
            kind,
            remaining: duration,
            total: duration,
        });
    }

    /// Get visible (non-expired) notifications.
    pub fn visible(&self) -> Vec<&Notification> {
        self.notifications
            .iter()
            .filter(|n| n.remaining > 0.0)
            .rev()
            .take(self.max_visible)
            .collect()
    }
}

/// A single toast notification.
pub struct Notification {
    pub text: String,
    pub kind: NotificationKind,
    pub remaining: f32,
    pub total: f32,
}

impl Notification {
    /// Opacity based on remaining time (fades out in last 0.5s).
    pub fn opacity(&self) -> f32 {
        if self.remaining > 0.5 {
            1.0
        } else {
            (self.remaining / 0.5).max(0.0)
        }
    }
}

/// Notification severity/type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationKind {
    Info,
    Success,
    Warning,
    Error,
    QuestUpdate,
    LevelUp,
    ItemReceived,
}

impl NotificationKind {
    /// Get the color associated with this notification type.
    pub fn color(&self) -> Color {
        match self {
            Self::Info => Color::srgb(0.8, 0.8, 0.9),
            Self::Success => Color::srgb(0.3, 0.9, 0.4),
            Self::Warning => Color::srgb(0.9, 0.8, 0.2),
            Self::Error => Color::srgb(0.9, 0.3, 0.3),
            Self::QuestUpdate => Color::srgb(0.4, 0.7, 1.0),
            Self::LevelUp => Color::srgb(1.0, 0.85, 0.1),
            Self::ItemReceived => Color::srgb(0.6, 0.9, 0.6),
        }
    }
}

// ---------------------------------------------------------------------------
// UI Marker Components
// ---------------------------------------------------------------------------

#[derive(Component)]
struct CraftingPanel;

#[derive(Component)]
struct QuestLogPanel;

#[derive(Component)]
struct SkillTreePanel;

#[derive(Component)]
struct ShopPanel;

#[derive(Component)]
struct NotificationContainer;

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

fn setup_panels(mut commands: Commands) {
    // Crafting panel (hidden)
    spawn_panel(
        &mut commands,
        "Crafting",
        Color::srgba(0.12, 0.10, 0.18, 0.90),
        Val::Px(60.0),
        Val::Px(80.0),
        300.0,
        400.0,
        CraftingPanel,
        vec![
            ("Recipes", 16.0, Color::srgb(0.9, 0.8, 0.3)),
            ("Select a recipe to craft.", 12.0, Color::srgba(0.7, 0.7, 0.7, 0.8)),
            ("Materials: ---", 12.0, Color::srgba(0.7, 0.7, 0.7, 0.8)),
            ("[C] Close", 11.0, Color::srgba(0.5, 0.5, 0.5, 0.6)),
        ],
    );

    // Quest log panel (hidden)
    spawn_panel(
        &mut commands,
        "Quest Log",
        Color::srgba(0.10, 0.12, 0.18, 0.90),
        Val::Px(60.0),
        Val::Px(80.0),
        300.0,
        400.0,
        QuestLogPanel,
        vec![
            ("Active Quests", 16.0, Color::srgb(0.4, 0.7, 1.0)),
            ("No active quests.", 12.0, Color::srgba(0.7, 0.7, 0.7, 0.8)),
            ("[J] Close", 11.0, Color::srgba(0.5, 0.5, 0.5, 0.6)),
        ],
    );

    // Skill tree panel (hidden)
    spawn_panel(
        &mut commands,
        "Skills",
        Color::srgba(0.15, 0.10, 0.10, 0.90),
        Val::Px(60.0),
        Val::Px(80.0),
        300.0,
        400.0,
        SkillTreePanel,
        vec![
            ("Skill Points: 0", 16.0, Color::srgb(0.9, 0.5, 0.2)),
            ("No skills available.", 12.0, Color::srgba(0.7, 0.7, 0.7, 0.8)),
            ("[K] Close", 11.0, Color::srgba(0.5, 0.5, 0.5, 0.6)),
        ],
    );

    // Shop panel (hidden)
    spawn_panel(
        &mut commands,
        "Shop",
        Color::srgba(0.10, 0.15, 0.10, 0.90),
        Val::Px(60.0),
        Val::Px(80.0),
        300.0,
        400.0,
        ShopPanel,
        vec![
            ("Items for Sale", 16.0, Color::srgb(0.9, 0.8, 0.3)),
            ("No items available.", 12.0, Color::srgba(0.7, 0.7, 0.7, 0.8)),
            ("Gold: 0", 14.0, Color::srgb(1.0, 0.85, 0.0)),
            ("[B] Close", 11.0, Color::srgba(0.5, 0.5, 0.5, 0.6)),
        ],
    );

    // Notification container (top-center, always visible)
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(50.0),
            left: Val::Percent(50.0),
            width: Val::Px(300.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        },
        NotificationContainer,
    ));
}

#[allow(clippy::too_many_arguments)]
fn spawn_panel<M: Component>(
    commands: &mut Commands,
    title: &str,
    bg_color: Color,
    left: Val,
    top: Val,
    width: f32,
    height: f32,
    marker: M,
    lines: Vec<(&str, f32, Color)>,
) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left,
                top,
                width: Val::Px(width),
                height: Val::Px(height),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                display: Display::None,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(bg_color),
            marker,
        ))
        .with_children(|parent| {
            // Title
            parent.spawn((
                Text::new(title),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
            ));

            // Content lines
            for (text, size, color) in lines {
                parent.spawn((
                    Text::new(text),
                    TextFont { font_size: size, ..default() },
                    TextColor(color),
                ));
            }
        });
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

type CraftingQuery = (With<CraftingPanel>, Without<QuestLogPanel>, Without<SkillTreePanel>, Without<ShopPanel>);
type QuestLogQuery = (With<QuestLogPanel>, Without<CraftingPanel>, Without<SkillTreePanel>, Without<ShopPanel>);
type SkillTreeQuery = (With<SkillTreePanel>, Without<CraftingPanel>, Without<QuestLogPanel>, Without<ShopPanel>);
type ShopQuery = (With<ShopPanel>, Without<CraftingPanel>, Without<QuestLogPanel>, Without<SkillTreePanel>);

fn handle_panel_events(
    mut states: ResMut<PanelStates>,
    mut events: EventReader<PanelEvent>,
    mut crafting_q: Query<&mut Node, CraftingQuery>,
    mut quest_q: Query<&mut Node, QuestLogQuery>,
    mut skill_q: Query<&mut Node, SkillTreeQuery>,
    mut shop_q: Query<&mut Node, ShopQuery>,
) {
    for event in events.read() {
        match event {
            PanelEvent::Toggle(kind) => {
                states.toggle(*kind);
                sync_panel_visibility(&states, &mut crafting_q, &mut quest_q, &mut skill_q, &mut shop_q);
            }
            PanelEvent::CloseAll => {
                states.close_all();
                sync_panel_visibility(&states, &mut crafting_q, &mut quest_q, &mut skill_q, &mut shop_q);
            }
            // Other events are handled by game systems that consume them
            _ => {}
        }
    }
}

fn sync_panel_visibility(
    states: &PanelStates,
    crafting_q: &mut Query<&mut Node, CraftingQuery>,
    quest_q: &mut Query<&mut Node, QuestLogQuery>,
    skill_q: &mut Query<&mut Node, SkillTreeQuery>,
    shop_q: &mut Query<&mut Node, ShopQuery>,
) {
    if let Ok(mut node) = crafting_q.get_single_mut() {
        node.display = if states.crafting_open { Display::Flex } else { Display::None };
    }
    if let Ok(mut node) = quest_q.get_single_mut() {
        node.display = if states.quest_log_open { Display::Flex } else { Display::None };
    }
    if let Ok(mut node) = skill_q.get_single_mut() {
        node.display = if states.skill_tree_open { Display::Flex } else { Display::None };
    }
    if let Ok(mut node) = shop_q.get_single_mut() {
        node.display = if states.shop_open { Display::Flex } else { Display::None };
    }
}

fn handle_panel_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut events: EventWriter<PanelEvent>,
) {
    if keyboard.just_pressed(KeyCode::KeyC) {
        events.send(PanelEvent::Toggle(PanelKind::Crafting));
    }
    if keyboard.just_pressed(KeyCode::KeyJ) {
        events.send(PanelEvent::Toggle(PanelKind::QuestLog));
    }
    if keyboard.just_pressed(KeyCode::KeyK) {
        events.send(PanelEvent::Toggle(PanelKind::SkillTree));
    }
    if keyboard.just_pressed(KeyCode::KeyB) {
        events.send(PanelEvent::Toggle(PanelKind::Shop));
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        events.send(PanelEvent::CloseAll);
    }
}

fn update_notifications(
    time: Res<Time>,
    mut queue: ResMut<NotificationQueue>,
) {
    let dt = time.delta_secs();
    for notification in queue.notifications.iter_mut() {
        notification.remaining -= dt;
    }
    queue.notifications.retain(|n| n.remaining > -0.5);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_states_default() {
        let states = PanelStates::default();
        assert!(!states.crafting_open);
        assert!(!states.quest_log_open);
        assert!(!states.skill_tree_open);
        assert!(!states.shop_open);
        assert!(states.active_panel.is_none());
        assert_eq!(states.open_count(), 0);
    }

    #[test]
    fn test_panel_toggle() {
        let mut states = PanelStates::default();
        states.toggle(PanelKind::Crafting);
        assert!(states.crafting_open);
        assert!(!states.quest_log_open);
        assert_eq!(states.open_count(), 1);
        assert_eq!(states.active_panel, Some(PanelKind::Crafting));

        // Toggle same panel closes it
        states.toggle(PanelKind::Crafting);
        assert!(!states.crafting_open);
        assert_eq!(states.open_count(), 0);
    }

    #[test]
    fn test_panel_toggle_switches() {
        let mut states = PanelStates::default();
        states.toggle(PanelKind::Crafting);
        assert!(states.crafting_open);

        // Opening quest log should close crafting
        states.toggle(PanelKind::QuestLog);
        assert!(!states.crafting_open);
        assert!(states.quest_log_open);
        assert_eq!(states.open_count(), 1);
    }

    #[test]
    fn test_close_all() {
        let mut states = PanelStates::default();
        states.toggle(PanelKind::SkillTree);
        assert_eq!(states.open_count(), 1);

        states.close_all();
        assert_eq!(states.open_count(), 0);
        assert!(states.active_panel.is_none());
    }

    #[test]
    fn test_notification_queue() {
        let mut queue = NotificationQueue::default();
        queue.push("Test message".into(), NotificationKind::Info);
        assert_eq!(queue.notifications.len(), 1);
        assert_eq!(queue.visible().len(), 1);
    }

    #[test]
    fn test_notification_push_timed() {
        let mut queue = NotificationQueue::default();
        queue.push_timed("Quick".into(), NotificationKind::Warning, 1.0);
        assert_eq!(queue.notifications[0].total, 1.0);
    }

    #[test]
    fn test_notification_opacity() {
        let n = Notification {
            text: "test".into(),
            kind: NotificationKind::Info,
            remaining: 2.0,
            total: 3.0,
        };
        assert_eq!(n.opacity(), 1.0);

        let fading = Notification {
            text: "test".into(),
            kind: NotificationKind::Info,
            remaining: 0.25,
            total: 3.0,
        };
        assert!(fading.opacity() < 1.0);
        assert!(fading.opacity() > 0.0);
    }

    #[test]
    fn test_notification_kind_colors() {
        let kinds = [
            NotificationKind::Info,
            NotificationKind::Success,
            NotificationKind::Warning,
            NotificationKind::Error,
            NotificationKind::QuestUpdate,
            NotificationKind::LevelUp,
            NotificationKind::ItemReceived,
        ];
        assert_eq!(kinds.len(), 7);
        for kind in &kinds {
            let _color = kind.color();
        }
    }

    #[test]
    fn test_panel_kind_variants() {
        let kinds = [
            PanelKind::Crafting,
            PanelKind::QuestLog,
            PanelKind::SkillTree,
            PanelKind::Shop,
        ];
        assert_eq!(kinds.len(), 4);
        assert_ne!(PanelKind::Crafting, PanelKind::Shop);
    }

    #[test]
    fn test_panel_events_all_variants() {
        let events = vec![
            PanelEvent::Toggle(PanelKind::Crafting),
            PanelEvent::CloseAll,
            PanelEvent::SelectRecipe { recipe_id: 1 },
            PanelEvent::CraftItem { recipe_id: 1 },
            PanelEvent::SelectQuest { quest_id: "q1".into() },
            PanelEvent::AbandonQuest { quest_id: "q1".into() },
            PanelEvent::AllocateSkill { skill_id: "s1".into() },
            PanelEvent::ShopBuy { item_id: 1, quantity: 2 },
            PanelEvent::ShopSell { item_id: 1, quantity: 1 },
        ];
        assert_eq!(events.len(), 9);
    }

    #[test]
    fn test_notification_overflow_trim() {
        let mut queue = NotificationQueue::default();
        // max_visible is 5, trim threshold is 5*2=10
        for i in 0..15 {
            queue.push(format!("Msg {}", i), NotificationKind::Info);
        }
        assert!(queue.notifications.len() <= 10);
    }
}
