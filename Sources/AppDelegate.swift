import AppKit
import Combine
import ServiceManagement
import SwiftUI

extension Notification.Name {
    static let showSetupGuide = Notification.Name("BoseBuddyShowSetupGuide")
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSPopoverDelegate {
    private let model = BoseController()
    private var statusItem: NSStatusItem!
    private var popover: NSPopover!
    private var cancellables = Set<AnyCancellable>()
    private var activityToken: NSObjectProtocol?
    private var bleProbe: BLEProbe?
    private var onboardingWindow: NSWindow?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        // Keep the 2-second connection watchdog from being App Napped; this is
        // a tiny menu bar utility, not a background batch job.
        activityToken = ProcessInfo.processInfo.beginActivity(
            options: .userInitiatedAllowingIdleSystemSleep,
            reason: "Monitor Bose headphone connection"
        )

        if CommandLine.arguments.contains("--ble-probe") {
            NSApp.activate(ignoringOtherApps: true)
            let probe = BLEProbe()
            bleProbe = probe
            probe.start()
            return
        }

        if CommandLine.arguments.contains("--selftest") {
            runSelfTest()
            return
        }

        if CommandLine.arguments.contains("--render") {
            runRender()
            return
        }

        if CommandLine.arguments.contains("--snapshot") {
            runSnapshot(demo: false)
            return
        }

        if CommandLine.arguments.contains("--snapshot-demo") {
            runSnapshot(demo: true)
            return
        }

        if CommandLine.arguments.contains("--snapshot-switch") {
            runSnapshot(demo: false, switchDemo: true)
            return
        }

        if CommandLine.arguments.contains("--snapshot-onboarding") {
            runOnboardingSnapshot()
            return
        }

        if CommandLine.arguments.contains("--login-test") {
            testLoginItem()
            return
        }

        if CommandLine.arguments.contains("--switch-test") {
            runSwitchTest()
            return
        }

        setUpStatusItem()
        model.onStatusChange = { [weak self] in self?.updateStatusItem() }
        updateStatusItem()

        model.objectWillChange
            .receive(on: RunLoop.main)
            .sink { [weak self] _ in
                DispatchQueue.main.async { self?.updateStatusItem() }
            }
            .store(in: &cancellables)

        NotificationCenter.default.addObserver(forName: .showSetupGuide, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor in self?.showOnboarding() }
        }

        let onboarded = UserDefaults.standard.bool(forKey: "hasCompletedOnboarding")
        let skipOnboarding = CommandLine.arguments.contains("--demo")

        if !onboarded && !skipOnboarding {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { [weak self] in
                self?.showOnboarding()
            }
        } else {
            model.start()
            if CommandLine.arguments.contains("--connect") {
                DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) { [weak self] in
                    Task { await self?.model.connect() }
                }
            }
            if CommandLine.arguments.contains("--demo") {
                DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) { [weak self] in
                    self?.showPopover()
                }
            }
        }
    }

    // MARK: - Onboarding

    private func showOnboarding() {
        if let window = onboardingWindow {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }
        let view = OnboardingView(model: model) { [weak self] in
            self?.finishOnboarding()
        }
        let hosting = NSHostingView(rootView: view)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 520, height: 600),
                              styleMask: [.titled, .closable, .fullSizeContentView],
                              backing: .buffered,
                              defer: false)
        window.title = "Welcome to BoseBuddy"
        window.titlebarAppearsTransparent = true
        window.isMovableByWindowBackground = true
        window.contentView = hosting
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        onboardingWindow = window
    }

    private func finishOnboarding() {
        UserDefaults.standard.set(true, forKey: "hasCompletedOnboarding")
        onboardingWindow?.close()
        onboardingWindow = nil
        model.start()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { [weak self] in
            self?.showPopover()
        }
    }

    private func runOnboardingSnapshot() {
        Task { @MainActor in
            let view = OnboardingView(model: model) {}
            let hosting = NSHostingView(rootView: view)
            hosting.frame = NSRect(x: 0, y: 0, width: 520, height: 600)
            let window = NSWindow(contentRect: hosting.frame,
                                  styleMask: [.titled],
                                  backing: .buffered,
                                  defer: false)
            window.contentView = hosting
            window.setFrameOrigin(NSPoint(x: 240, y: 240))
            NSApp.activate(ignoringOtherApps: true)
            window.makeKeyAndOrderFront(nil)
            try? await Task.sleep(nanoseconds: 1_500_000_000)
            capture(hosting, to: "/tmp/bosebuddy-onboarding.png")
            window.orderOut(nil)
            NSApp.terminate(nil)
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        model.popoverClosed()
        model.disconnect()
        if let activityToken {
            ProcessInfo.processInfo.endActivity(activityToken)
        }
    }

    // MARK: - Status item

    private func setUpStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        guard let button = statusItem.button else { return }
        button.image = Self.menuIcon()
        button.image?.isTemplate = true
        button.imagePosition = .imageLeading
        button.imageScaling = .scaleProportionallyDown
        button.font = NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .medium)
        button.target = self
        button.action = #selector(togglePopover(_:))
        button.toolTip = "BoseBuddy — Bose headphones"
    }

    /// Bose headphone mark as a template image, so macOS tints it black in the
    /// light menu bar and white in the dark one.
    private static func menuIcon() -> NSImage? {
        guard let image = Bundle.main.image(forResource: "MenuIcon") else { return nil }
        image.isTemplate = true
        image.size = NSSize(width: 18, height: 18)
        return image
    }

    private func updateStatusItem() {
        guard let button = statusItem?.button else { return }
        if model.linkState.isConnected {
            button.appearsDisabled = false
            if model.showBatteryInMenuBar, let battery = model.battery {
                button.title = " \(battery)%"
            } else {
                button.title = ""
            }
        } else {
            button.title = ""
            button.appearsDisabled = !model.linkState.isConnecting
        }
    }

    // MARK: - Popover

    private func ensurePopover() -> NSPopover {
        if let popover { return popover }
        let popover = NSPopover()
        popover.behavior = .transient
        popover.animates = true
        popover.contentSize = NSSize(width: 360, height: 562)
        popover.delegate = self
        popover.contentViewController = NSHostingController(rootView: PopoverView(model: model))
        self.popover = popover
        return popover
    }

    @objc private func togglePopover(_ sender: Any?) {
        if popover?.isShown == true {
            popover?.performClose(nil)
        } else {
            showPopover()
        }
    }

    private func showPopover() {
        guard let button = statusItem.button else { return }
        let popover = ensurePopover()
        NSApp.activate(ignoringOtherApps: true)
        popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
        model.popoverOpened()
    }

    func popoverDidClose(_ notification: Notification) {
        model.popoverClosed()
        // Release the SwiftUI popover while idle; recreate it on next click.
        DispatchQueue.main.async { [weak self] in
            self?.popover?.contentViewController = nil
            self?.popover = nil
        }
    }

    private func runSwitchTest() {
        Task { @MainActor in
            let fragment = CommandLine.arguments.last.flatMap { $0.hasPrefix("--") ? nil : $0 } ?? "iphone"
            print("BoseBuddy switch test → \(fragment)")
            model.refreshPairedDevices()
            await model.connect()
            await model.runSwitchTest(to: fragment)
            model.disconnect()
            NSApp.terminate(nil)
        }
    }

    private func testLoginItem() {
        Task { @MainActor in
            print("login status before: \(SMAppService.mainApp.status.rawValue)")
            do {
                try SMAppService.mainApp.register()
                print("registered: \(SMAppService.mainApp.status.rawValue)")
            } catch {
                print("register failed: \(error.localizedDescription)")
            }
            do {
                try SMAppService.mainApp.unregister()
                print("unregistered: \(SMAppService.mainApp.status.rawValue)")
            } catch {
                print("unregister failed: \(error.localizedDescription)")
            }
            NSApp.terminate(nil)
        }
    }

    // MARK: - Window snapshot

    private func runSnapshot(demo: Bool, switchDemo: Bool = false) {
        Task { @MainActor in
            if switchDemo {
                model.loadDemoSwitchState()
            } else if demo {
                model.loadDemoState()
            } else {
                model.refreshPairedDevices()
                await model.connect()
                await model.refresh(deep: true)
            }

            let hosting = NSHostingView(rootView: PopoverView(model: model))
            hosting.frame = NSRect(x: 0, y: 0, width: 360, height: 562)
            let window = NSWindow(contentRect: hosting.frame,
                                  styleMask: [.borderless],
                                  backing: .buffered,
                                  defer: false)
            window.contentView = hosting
            window.backgroundColor = .windowBackgroundColor
            window.isOpaque = true
            window.setFrameOrigin(NSPoint(x: 240, y: 240))
            window.orderFrontRegardless()
            NSApp.activate(ignoringOtherApps: true)
            try? await Task.sleep(nanoseconds: 1_800_000_000)

            capture(hosting, to: "/tmp/bosebuddy-window.png")
            window.orderOut(nil)
            model.disconnect()
            NSApp.terminate(nil)
        }
    }

    private func capture(_ view: NSView, to path: String) {
        guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else {
            print("snapshot: no bitmap rep")
            return
        }
        view.cacheDisplay(in: view.bounds, to: rep)
        if let data = rep.representation(using: .png, properties: [:]) {
            try? data.write(to: URL(fileURLWithPath: path))
            print("snapshot: \(path) multipoint=\(model.multipoint) peers=\(model.peers.count)")
        }
    }

    // MARK: - Snapshot renderer

    private func runRender() {
        Task { @MainActor in
            model.refreshPairedDevices()
            await model.connect()
            await model.refresh(deep: true)
            let renderer = ImageRenderer(
                content: PopoverView(model: model)
                    .environment(\.boseBarRenderMode, true)
                    .frame(width: 360, height: 780)
            )
            renderer.scale = 2
            if let cgImage = renderer.cgImage {
                let rep = NSBitmapImageRep(cgImage: cgImage)
                if let data = rep.representation(using: .png, properties: [:]) {
                    try? data.write(to: URL(fileURLWithPath: "/tmp/bosebuddy-render.png"))
                    print("rendered /tmp/bosebuddy-render.png")
                }
            } else {
                print("render failed")
            }
            model.disconnect()
            NSApp.terminate(nil)
        }
    }

    // MARK: - Self test

    private func runSelfTest() {
        Task { @MainActor in
            print("BoseBuddy self-test")
            model.refreshPairedDevices()
            print("paired headphones: \(model.headphones.map { "\($0.name) [\($0.address)] connected=\($0.connected)" })")
            await model.connect()
            print(model.debugSummary)
            if CommandLine.arguments.contains("--actions") {
                await model.runActionTest()
            }
            model.disconnect()
            print("done")
            NSApp.terminate(nil)
        }
    }
}
