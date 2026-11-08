import AppKit
import Combine
import CoreBluetooth
import SwiftUI

struct OnboardingView: View {
    @ObservedObject var model: BoseController
    var onFinish: () -> Void

    @State private var step = 0
    @State private var pairing = false
    @State private var pairError: String?
    @State private var tick = 0

    private var authorization: CBManagerAuthorization { CBCentralManager.authorization }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider().opacity(0.6)
            Group {
                switch step {
                case 0: welcomeStep
                case 1: permissionStep
                case 2: pairStep
                default: readyStep
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .padding(.horizontal, 30)
            .padding(.vertical, 24)
            Divider().opacity(0.6)
            footer
        }
        .frame(width: 520, height: 600)
        .background(Color(nsColor: .windowBackgroundColor))
        .tint(Theme.accent)
        .onReceive(Timer.publish(every: 1, on: .main, in: .common).autoconnect()) { _ in
            tick += 1
        }
    }

    // MARK: - Header

    private var header: some View {
        HStack(spacing: 12) {
            ZStack {
                RoundedRectangle(cornerRadius: 11, style: .continuous)
                    .fill(Theme.accent.opacity(0.14))
                Image(systemName: "headphones")
                    .font(.system(size: 19, weight: .semibold))
                    .foregroundStyle(Theme.accent)
            }
            .frame(width: 40, height: 40)

            VStack(alignment: .leading, spacing: 1) {
                Text("BoseBuddy")
                    .font(.system(size: 15, weight: .semibold))
                Text("Setup")
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
            }

            Spacer()

            HStack(spacing: 6) {
                ForEach(0..<4, id: \.self) { index in
                    Capsule()
                        .fill(index == step ? Theme.accent : Color.primary.opacity(0.12))
                        .frame(width: index == step ? 18 : 7, height: 7)
                        .animation(.easeInOut(duration: 0.2), value: step)
                }
            }
        }
        .padding(.horizontal, 22)
        .padding(.vertical, 14)
    }

    // MARK: - Steps

    private var welcomeStep: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Your Bose headphones,\nin the menu bar.")
                .font(.system(size: 26, weight: .semibold))
                .fixedSize(horizontal: false, vertical: true)

            Text("Control your QuietComfort headphones without the phone app, a Bose account, or the cloud.")
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            VStack(alignment: .leading, spacing: 10) {
                feature("point.3.connected.trianglepath.dotted",
                        "Switch audio devices",
                        "Connect, disconnect, or switch between the devices paired to your headphones.")
                feature("wave.3.right",
                        "BLE control",
                        "Control works over Bluetooth LE — the Mac doesn’t need to use one of the two audio slots.")
                feature("battery.75",
                        "Battery, modes, EQ",
                        "See battery and active audio, switch noise-control modes, adjust EQ, and more.")
            }
            .padding(.top, 4)

            HStack(spacing: 6) {
                Image(systemName: "lock.shield")
                    .font(.system(size: 10))
                Text("Unsigned build — macOS may ask you to confirm the first launch. That’s expected.")
                    .font(.system(size: 10))
                    .fixedSize(horizontal: false, vertical: true)
            }
            .foregroundStyle(.tertiary)

            Spacer()

            Button {
                withAnimation { step = 1 }
            } label: {
                Text("Get started").frame(minWidth: 110)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        }
    }

    private var permissionStep: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Allow Bluetooth.")
                .font(.system(size: 26, weight: .semibold))

            Text("BoseBuddy talks to your headphones directly over Bluetooth. macOS needs your permission the first time.")
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            permissionStatus

            Text("If you don’t see the prompt, open System Settings → Privacy & Security → Bluetooth and switch BoseBuddy on.")
                .font(.system(size: 11))
                .foregroundStyle(.tertiary)
                .fixedSize(horizontal: false, vertical: true)

            Spacer()

            HStack(spacing: 10) {
                Button("Open Bluetooth Settings") {
                    model.openBluetoothSettings()
                }
                .buttonStyle(.bordered)

                Button {
                    withAnimation { step = 2 }
                } label: {
                    Text("Continue").frame(minWidth: 90)
                }
                .buttonStyle(.borderedProminent)
            }
        }
    }

    private var permissionStatus: some View {
        HStack(spacing: 9) {
            Image(systemName: permissionSymbol)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(permissionColor)
            Text(permissionText)
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(11)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 9, style: .continuous)
                .fill(permissionColor.opacity(0.09))
        )
    }

    private var permissionSymbol: String {
        switch authorization {
        case .allowedAlways: return "checkmark.circle.fill"
        case .denied: return "exclamationmark.triangle.fill"
        case .restricted: return "lock.fill"
        default: return "hand.raised.fill"
        }
    }

    private var permissionColor: Color {
        switch authorization {
        case .allowedAlways: return .green
        case .denied, .restricted: return .orange
        default: return .blue
        }
    }

    private var permissionText: String {
        switch authorization {
        case .allowedAlways:
            return "Bluetooth access is granted."
        case .denied:
            return "Bluetooth access is off. Enable BoseBuddy in System Settings → Privacy & Security → Bluetooth."
        case .restricted:
            return "Bluetooth is restricted on this Mac."
        default:
            return "You’ll see a macOS prompt when BoseBuddy first connects to the headphones. Choose Allow."
        }
    }

    private var pairStep: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Pair your headphones.")
                .font(.system(size: 26, weight: .semibold))

            Text("This is a one-time Bluetooth LE pairing. Afterwards BoseBuddy can control the headphones without using an audio slot.")
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            VStack(alignment: .leading, spacing: 9) {
                numbered(1, "Put the headphones in pairing mode — hold the Bluetooth button until the light blinks.")
                numbered(2, "Keep them close to the Mac.")
                numbered(3, "Click Pair now and allow the macOS Bluetooth prompt if it appears.")
            }

            if model.linkState.isConnected {
                statusLine("checkmark.circle.fill", .green, "Connected to \(model.deviceName).")
            } else if pairing {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("Looking for the headphones…")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }
                .padding(.top, 2)
            } else if model.needsBLEBonding {
                statusLine("exclamationmark.triangle.fill", .orange,
                           "Pairing didn’t complete. Make sure the light is blinking, then try again.")
            } else if let pairError {
                statusLine("exclamationmark.triangle.fill", .orange, pairError)
            }

            Spacer()

            HStack(spacing: 10) {
                Button {
                    pairNow()
                } label: {
                    Text(pairing ? "Pairing…" : "Pair now")
                        .frame(minWidth: 90)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(pairing)

                if model.linkState.isConnected {
                    Button {
                        withAnimation { step = 3 }
                    } label: {
                        Text("Continue").frame(minWidth: 80)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)
                }
            }

            Text("You can do this later — BoseBuddy will show the same steps in its menu.")
                .font(.system(size: 10))
                .foregroundStyle(.tertiary)
        }
    }

    private var readyStep: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("You’re all set.")
                .font(.system(size: 26, weight: .semibold))

            Text("BoseBuddy lives in the menu bar. Click the Bose logo any time to switch devices, change modes, or connect this Mac’s audio.")
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            HStack(spacing: 10) {
                ZStack {
                    RoundedRectangle(cornerRadius: 9, style: .continuous)
                        .fill(Color.primary.opacity(0.06))
                    if let icon = Bundle.main.image(forResource: "MenuIcon") {
                        Image(nsImage: icon)
                            .resizable()
                            .interpolation(.high)
                            .frame(width: 22, height: 22)
                    } else {
                        Image(systemName: "headphones")
                    }
                }
                .frame(width: 38, height: 38)

                Text("Look for this icon in the menu bar, next to the battery percentage.")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(11)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .fill(Color.primary.opacity(0.035))
            )

            Spacer()

            Button {
                onFinish()
            } label: {
                Text("Finish").frame(minWidth: 110)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        }
    }

    // MARK: - Footer

    private var footer: some View {
        HStack(spacing: 12) {
            if step > 0, step < 3 {
                Button("Back") {
                    withAnimation { step -= 1 }
                }
                .buttonStyle(.borderless)
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
            }

            Spacer()

            if step < 3 {
                Button("Skip setup") {
                    onFinish()
                }
                .buttonStyle(.borderless)
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 22)
        .padding(.vertical, 12)
    }

    // MARK: - Helpers

    private func feature(_ symbol: String, _ title: String, _ body: String) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: symbol)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(Theme.accent)
                .frame(width: 18)
            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                    .font(.system(size: 12, weight: .semibold))
                Text(body)
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private func numbered(_ number: Int, _ text: String) -> some View {
        HStack(alignment: .top, spacing: 9) {
            Text("\(number)")
                .font(.system(size: 10, weight: .bold, design: .rounded))
                .foregroundStyle(.white)
                .frame(width: 17, height: 17)
                .background(Circle().fill(Theme.accent))
            Text(text)
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func statusLine(_ symbol: String, _ color: Color, _ text: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: symbol)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(color)
            Text(text)
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.top, 2)
    }

    private func pairNow() {
        guard !pairing else { return }
        pairing = true
        pairError = nil
        model.preferBLE = true
        Task { @MainActor in
            await model.connect()
            pairing = false
            if model.linkState.isConnected {
                withAnimation { step = 3 }
            } else {
                pairError = model.errorText ?? "Couldn’t find the headphones. Make sure they’re in pairing mode."
            }
        }
    }
}
