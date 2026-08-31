-- agents-tui swarm dashboard -- Hyprland integration.
--
-- Append to ~/.config/hypr/bindings.lua (the binding) and
-- ~/.config/hypr/looknfeel.lua or hyprland.lua (the window rules), or let
-- omarchy/install.sh do it.

-- SUPER + A brings the dashboard up over whatever is in front and dismisses
-- it again. A special workspace rather than a tiled window because on a small
-- display a permanently-visible dashboard costs area that agent output needs.
o.bind("SUPER + A", "Agent swarm dashboard", "omarchy-agents-dashboard")

-- The dashboard is dense text with meters in it; Omarchy's default 0.985/0.96
-- window opacity puts the wallpaper behind small type. Force it opaque.
o.window("agents-dashboard", {
  opacity = "1.0 1.0",
  tag = "-default-opacity",
})
