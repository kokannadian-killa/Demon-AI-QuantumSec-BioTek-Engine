// DemonOS PhantomCore default desktop.
// The top panel and the dock are Garuda Dr460nized's own layout templates
// (org.garuda.desktop.defaultPanel / defaultDock from garuda-dr460nized,
// GPL-3.0-or-later); DemonOS only swaps the start icon, pins Lillith in the dock
// and sets the DemonOS wallpaper.
var plasma = getApiVersion(1)

const krunner = ConfigFile('krunnerrc')
krunner.group = 'General'
krunner.writeEntry('FreeFloating', true);

loadTemplate("org.garuda.desktop.defaultPanel")
loadTemplate("org.garuda.desktop.defaultDock")

var ps = panels();
for (var i = 0; i < ps.length; i++) {
    var ws = ps[i].widgets();
    for (var j = 0; j < ws.length; j++) {
        var w = ws[j];
        if (w.type == "org.kde.plasma.kickoff") {
            w.currentConfigGroup = ["General"];
            w.writeConfig("icon", "/usr/share/icons/demonos/demonos-start.png");
        }
        if (w.type == "org.kde.plasma.icontasks" || w.type == "org.kde.plasma.taskmanager") {
            w.currentConfigGroup = ["General"];
            var l = w.readConfig("launchers", "");
            var s = (typeof l === "string") ? l : l.join(",");
            if (s.indexOf("lillith.desktop") < 0)
                w.writeConfig("launchers", "applications:lillith.desktop" + (s ? "," + s : ""));
        }
    }
}

var ds = desktops();
for (var k = 0; k < ds.length; k++) {
    var d = ds[k];
    d.wallpaperPlugin = "org.kde.image";
    d.currentConfigGroup = ["Wallpaper", "org.kde.image", "General"];
    d.writeConfig("Image", "file:///usr/share/wallpapers/DemonOS/contents/images/1920x1080.png");
}
