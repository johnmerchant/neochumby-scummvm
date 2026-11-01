/* neochumby-scummvm: the OSystem backend for the Chumby Classic.
 *
 * Structured after ScummVM's backends/platform/null/null.cpp, and so kept
 * under ScummVM's licence, not the MIT licence of this repository's Rust code.
 *
 * ScummVM is the legal property of its developers, whose names
 * are too numerous to list here. Please refer to the COPYRIGHT
 * file distributed with this source distribution.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

// Only the interface to ScummVM is here. The screen, touch, top button, speaker
// and the save quota are src/backend.rs (the chum_* functions).

#define FORBIDDEN_SYMBOL_ALLOW_ALL

#include <pthread.h>
#include <stdio.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

#include "common/scummsys.h"

#include "audio/mixer_intern.h"
#include "backends/events/default/default-events.h"
#include "backends/fs/posix/posix-fs-factory.h"
#include "backends/graphics/null/null-graphics.h"
#include "backends/mixer/mixer.h"
#include "backends/modular-backend.h"
#include "backends/saves/default/default-saves.h"
#include "backends/timer/default/default-timer.h"
#include "base/main.h"
#include "common/config-manager.h"
#include "common/events.h"
#include "common/memstream.h"
#include "common/mutex.h"
#include "engines/engine.h"
#include "graphics/surface.h"

struct ChumFrame {
	const byte *screen;
	int32 pitch, w, h;
	const uint16 *palette;
	const uint16 *overlay;
	int32 overlayOn;
	int32 dirtyTop, dirtyBottom; // Rows of the source; top >= bottom: none.
	int32 shake;
	const byte *cursor;
	int32 cursorW, cursorH, cursorX, cursorY;
	uint32 cursorKey;
	const uint16 *cursorPalette;
	int32 cursorOn;
};

struct ChumEvent {
	int32 kind, x, y, key, ascii;
};

enum {
	kChumNone, kChumMove, kChumLDown, kChumLUp, kChumRDown, kChumRUp,
	kChumKeyDown, kChumKeyUp, kChumMenu, kChumQuit
};

extern "C" {
void chum_present(const ChumFrame *frame);
int32 chum_poll(ChumEvent *event, int32 overlayOn, int32 gameH);
void chum_warp(int32 x, int32 y);
void chum_audio_open(int32 rate, int32 channels);
void chum_audio_write(const int16 *pcm, int32 samples);
int32 chum_audio_buffered();
void chum_shutdown();
int32 chum_storage_fits(const char *dir, const char *name, int32 size);
byte *chum_gzip(const byte *data, int32 len, int32 *out);
void chum_gzip_free(byte *data, int32 len);
}

/** The overlay stops above the soft buttons, which stay usable in the GUI. */
static const int kOverlayW = 320, kOverlayH = 220;
/** Mono, at the rate most of these games' sounds are: the least work. */
static const uint kAudioRate = 11025, kAudioChunk = 512, kAudioAhead = 2048;

class PthreadMutex final : public Common::MutexInternal {
public:
	PthreadMutex() {
		pthread_mutexattr_t attr;
		pthread_mutexattr_init(&attr);
		pthread_mutexattr_settype(&attr, PTHREAD_MUTEX_RECURSIVE);
		pthread_mutex_init(&_mutex, &attr);
		pthread_mutexattr_destroy(&attr);
	}
	~PthreadMutex() override { pthread_mutex_destroy(&_mutex); }
	bool lock() override { return pthread_mutex_lock(&_mutex) == 0; }
	bool unlock() override { return pthread_mutex_unlock(&_mutex) == 0; }

private:
	pthread_mutex_t _mutex;
};

class ChumbyGraphics final : public NullGraphicsManager {
public:
	ChumbyGraphics() : _overlayOn(false), _mouseOn(false), _mouseX(0), _mouseY(0), _hotX(0), _hotY(0),
		_cursor(nullptr), _cursorW(0), _cursorH(0), _cursorKey(0), _cursorPaletteOn(false), _shake(0),
		_changeId(0), _dirtyTop(0), _dirtyBottom(0), _overlay(new uint16[kOverlayW * kOverlayH]()),
		_lastCursorTop(0), _lastCursorBottom(0) {
		memset(_rgb, 0, sizeof(_rgb));
		memset(_palette, 0, sizeof(_palette));
		memset(_cursorPalette, 0, sizeof(_cursorPalette));
		_screen.create(320, 200, Graphics::PixelFormat::createFormatCLUT8());
	}
	~ChumbyGraphics() override {
		_screen.free();
		delete[] _overlay;
		delete[] _cursor;
	}

	bool overlayOn() const { return _overlayOn; }
	void mouseEvent(int x, int y) { _mouseX = x; _mouseY = y; }

	bool hasFeature(OSystem::Feature f) const override { return f == OSystem::kFeatureCursorPalette; }
	void setFeatureState(OSystem::Feature f, bool enable) override {
		if (f == OSystem::kFeatureCursorPalette)
			_cursorPaletteOn = enable;
	}
	bool getFeatureState(OSystem::Feature f) const override {
		return f == OSystem::kFeatureCursorPalette && _cursorPaletteOn;
	}

	// A mode larger than the panel fails the transaction, so that an engine
	// offering a list (Dreamweb's titles) falls back on one that fits.
	void beginGFXTransaction() override { _sizeFailed = false; }
	OSystem::TransactionError endGFXTransaction() override {
		return _sizeFailed ? OSystem::kTransactionSizeChangeFailed : OSystem::kTransactionSuccess;
	}

	void initSize(uint width, uint height, const Graphics::PixelFormat *) override {
		if (width > 320 || height > 240) {
			_sizeFailed = true;
			return;
		}
		if ((int)width != _screen.w || (int)height != _screen.h) {
			_screen.free();
			_screen.create(width, height, Graphics::PixelFormat::createFormatCLUT8());
			_changeId++;
		}
		dirtyAll();
	}
	int getScreenChangeID() const override { return _changeId; }
	int16 getHeight() const override { return _screen.h; }
	int16 getWidth() const override { return _screen.w; }

	void setPalette(const byte *colors, uint start, uint num) override {
		memcpy(_rgb + start * 3, colors, num * 3);
		for (uint i = start; i < start + num; i++, colors += 3)
			_palette[i] = rgb565(colors[0], colors[1], colors[2]);
		dirtyAll();
	}
	void grabPalette(byte *colors, uint start, uint num) const override { memcpy(colors, _rgb + start * 3, num * 3); }

	void copyRectToScreen(const void *buf, int pitch, int x, int y, int w, int h) override {
		_screen.copyRectToSurface(buf, pitch, x, y, w, h);
		dirtyRows(y, y + h);
	}
	Graphics::Surface *lockScreen() override { return &_screen; }
	void unlockScreen() override { dirtyAll(); }
	void fillScreen(uint32 col) override { _screen.fillRect(Common::Rect(_screen.w, _screen.h), col); dirtyAll(); }
	void fillScreen(const Common::Rect &r, uint32 col) override { _screen.fillRect(r, col); dirtyRows(r.top, r.bottom); }
	void setShakePos(int, int y) override {
		if (y != _shake) {
			_shake = y;
			dirtyAll();
		}
	}

	void updateScreen() override {
		ChumFrame f;
		f.screen = (const byte *)_screen.getPixels();
		f.pitch = _screen.pitch;
		f.w = _screen.w;
		f.h = _screen.h;
		f.palette = _palette;
		f.overlay = _overlay;
		f.overlayOn = _overlayOn;
		f.shake = _shake;
		f.cursor = _cursor;
		f.cursorW = _cursorW;
		f.cursorH = _cursorH;
		f.cursorX = _mouseX - _hotX;
		f.cursorY = _mouseY - _hotY;
		f.cursorKey = _cursorKey;
		f.cursorPalette = _cursorPaletteOn ? _cursorPalette : _palette;
		f.cursorOn = _mouseOn && _cursor;
		// The rows the cursor was on are drawn again, and those it is on now.
		int top = f.cursorY, bottom = f.cursorY + _cursorH;
		if (f.cursorOn)
			dirtyRows(top, bottom);
		dirtyRows(_lastCursorTop, _lastCursorBottom);
		_lastCursorTop = f.cursorOn ? top : 0;
		_lastCursorBottom = f.cursorOn ? bottom : 0;
		f.dirtyTop = _dirtyTop;
		f.dirtyBottom = _dirtyBottom;
		_dirtyTop = _dirtyBottom = 0;
		chum_present(&f);
	}

	void showOverlay(bool) override { _overlayOn = true; dirtyAll(); }
	void hideOverlay() override { _overlayOn = false; dirtyAll(); }
	bool isOverlayVisible() const override { return _overlayOn; }
	Graphics::PixelFormat getOverlayFormat() const override { return Graphics::PixelFormat(2, 5, 6, 5, 0, 11, 5, 0, 0); }
	void clearOverlay() override {
		// The game, under the GUI.
		memset(_overlay, 0, kOverlayW * kOverlayH * 2);
		int w = MIN<int>(_screen.w, kOverlayW), h = MIN<int>(_screen.h, kOverlayH);
		for (int y = 0; y < h; y++) {
			const byte *s = (const byte *)_screen.getBasePtr(0, y);
			uint16 *d = _overlay + y * kOverlayW;
			for (int x = 0; x < w; x++)
				d[x] = _palette[s[x]];
		}
		dirtyAll();
	}
	void grabOverlay(Graphics::Surface &surface) const override {
		for (int y = 0; y < kOverlayH && y < surface.h; y++)
			memcpy(surface.getBasePtr(0, y), _overlay + y * kOverlayW, MIN<int>(surface.w, kOverlayW) * 2);
	}
	void copyRectToOverlay(const void *buf, int pitch, int x, int y, int w, int h) override {
		const byte *src = (const byte *)buf;
		if (x < 0) {
			src -= x * 2;
			w += x;
			x = 0;
		}
		if (y < 0) {
			src -= y * pitch;
			h += y;
			y = 0;
		}
		w = MIN(w, kOverlayW - x);
		h = MIN(h, kOverlayH - y);
		if (w <= 0 || h <= 0)
			return;
		for (int r = 0; r < h; r++, src += pitch)
			memcpy(_overlay + (y + r) * kOverlayW + x, src, w * 2);
		dirtyAll();
	}
	int16 getOverlayHeight() const override { return kOverlayH; }
	int16 getOverlayWidth() const override { return kOverlayW; }

	bool showMouse(bool visible) override {
		bool was = _mouseOn;
		_mouseOn = visible;
		return was;
	}
	void warpMouse(int x, int y) override {
		_mouseX = x;
		_mouseY = y;
		chum_warp(x, y);
	}
	void setMouseCursor(const void *buf, uint w, uint h, int hotX, int hotY, uint32 key, const Graphics::PixelFormat *, const byte *, frac_t, frac_t) override {
		delete[] _cursor;
		_cursor = new byte[w * h];
		memcpy(_cursor, buf, w * h);
		_cursorW = w;
		_cursorH = h;
		_hotX = hotX;
		_hotY = hotY;
		_cursorKey = key;
	}
	void setCursorPalette(const byte *colors, uint start, uint num) override {
		for (uint i = start; i < start + num; i++, colors += 3)
			_cursorPalette[i] = rgb565(colors[0], colors[1], colors[2]);
		_cursorPaletteOn = true;
	}

private:
	static uint16 rgb565(byte r, byte g, byte b) { return (r & 0xF8) << 8 | (g & 0xFC) << 3 | b >> 3; }
	void dirtyRows(int top, int bottom) {
		if (top >= bottom)
			return;
		if (_dirtyTop >= _dirtyBottom) {
			_dirtyTop = top;
			_dirtyBottom = bottom;
		} else {
			_dirtyTop = MIN(_dirtyTop, top);
			_dirtyBottom = MAX(_dirtyBottom, bottom);
		}
	}
	void dirtyAll() { dirtyRows(0, 240); }

	Graphics::Surface _screen;
	bool _sizeFailed = false;
	bool _overlayOn, _mouseOn;
	int _mouseX, _mouseY, _hotX, _hotY;
	byte *_cursor;
	int _cursorW, _cursorH;
	uint32 _cursorKey;
	bool _cursorPaletteOn;
	int _shake, _changeId, _dirtyTop, _dirtyBottom;
	uint16 *_overlay;
	int _lastCursorTop, _lastCursorBottom;
	byte _rgb[768];
	uint16 _palette[256], _cursorPalette[256];
};

class ChumbyMixer final : public MixerManager {
public:
	void init() override {
		_mixer = new Audio::MixerImpl(kAudioRate, false, kAudioChunk);
		_mixer->setReady(true);
		chum_audio_open(kAudioRate, 1);
		pthread_t thread;
		pthread_create(&thread, nullptr, run, this);
		pthread_detach(thread);
	}
	void suspendAudio() override { _audioSuspended = true; }
	int resumeAudio() override {
		if (!_audioSuspended)
			return -2;
		_audioSuspended = false;
		return 0;
	}
	bool isNullDevice() const override { return false; }

private:
	// Keeps a little mixed ahead of the speaker. The stream never blocks, so
	// its clock paces this.
	static void *run(void *arg) {
		ChumbyMixer *self = (ChumbyMixer *)arg;
		static int16 buf[kAudioChunk];
		for (;;) {
			if (self->_audioSuspended || chum_audio_buffered() >= (int32)kAudioAhead) {
				usleep(10000);
				continue;
			}
			self->_mixer->mixCallback((byte *)buf, sizeof(buf));
			chum_audio_write(buf, kAudioChunk);
		}
		return nullptr;
	}
};

class ChumbySaves;

/**
 * A save held in memory until it is finished, then written only if it fits the
 * app's storage: the session ends an app that goes over its limit, so a save
 * that does not fit fails, as the engine reports, instead.
 */
class QuotaWriteStream final : public Common::WriteStream {
public:
	QuotaWriteStream(ChumbySaves *manager, const Common::String &name, bool compress)
		: _manager(manager), _name(name), _compress(compress), _memory(DisposeAfterUse::YES), _failed(false), _done(false) {}
	~QuotaWriteStream() override { finalize(); }

	bool err() const override { return _failed; }
	void clearErr() override { _failed = false; }
	uint32 write(const void *ptr, uint32 size) override { return _memory.write(ptr, size); }
	int64 pos() const override { return _memory.pos(); }
	bool flush() override { return !_failed; }
	void finalize() override;

private:
	ChumbySaves *_manager;
	Common::String _name;
	bool _compress;
	Common::MemoryWriteStreamDynamic _memory;
	bool _failed, _done;
};

class ChumbySaves final : public DefaultSaveFileManager {
public:
	Common::OutSaveFile *openForSaving(const Common::String &name, bool compress) override {
		return new Common::OutSaveFile(new QuotaWriteStream(this, name, compress));
	}
	Common::OutSaveFile *openReal(const Common::String &name, bool compress) {
		return DefaultSaveFileManager::openForSaving(name, compress);
	}
};

void QuotaWriteStream::finalize() {
	if (_done)
		return;
	_done = true;
	const byte *data = _memory.getData();
	int32 size = _memory.size(), gzSize = 0;
	byte *gz = nullptr;
	if (_compress) {
		gz = chum_gzip(data, size, &gzSize);
		data = gz;
		size = gzSize;
	}
	Common::String dir = ConfMan.getPath("savepath").toString(Common::Path::kNativeSeparator);
	if (!chum_storage_fits(dir.c_str(), _name.c_str(), size)) {
		warning("%s: %d bytes do not fit in the app's storage", _name.c_str(), (int)size);
		_failed = true;
	} else {
		// The old copy goes first, so that the two together never count
		// against the limit.
		_manager->removeSavefile(_name);
		Common::OutSaveFile *out = _manager->openReal(_name, false);
		if (out) {
			out->write(data, size);
			out->finalize();
			_failed = out->err();
			delete out;
		} else {
			_failed = true;
		}
	}
	if (gz)
		chum_gzip_free(gz, gzSize);
}

class OSystem_Chumby final : public ModularMixerBackend, public ModularGraphicsBackend, Common::EventSource {
public:
	OSystem_Chumby() : _sawEngine(false) {
		_fsFactory = new POSIXFilesystemFactory();
		gettimeofday(&_start, nullptr);
	}

	void initBackend() override {
		_timerManager = new DefaultTimerManager();
		_eventManager = new DefaultEventManager(this);
		_savefileManager = new ChumbySaves();
		_graphicsManager = new ChumbyGraphics();
		_mixerManager = new ChumbyMixer();
		_mixerManager->init();
		pthread_t thread;
		pthread_create(&thread, nullptr, timers, this);
		pthread_detach(thread);
		BaseBackend::initBackend();
	}

	bool pollEvent(Common::Event &event) override {
		// Without a game, ScummVM would open its desktop launcher: the list
		// is ours, so its "Return to Launcher" ends this process, and ours
		// shows it.
		if (g_engine)
			_sawEngine = true;
		else if (_sawEngine)
			quit();

		ChumbyGraphics *graphics = (ChumbyGraphics *)_graphicsManager;
		ChumEvent e;
		if (!chum_poll(&e, graphics->overlayOn(), graphics->getHeight()))
			return false;
		event.mouse = Common::Point(e.x, e.y);
		switch (e.kind) {
		case kChumMove: event.type = Common::EVENT_MOUSEMOVE; break;
		case kChumLDown: event.type = Common::EVENT_LBUTTONDOWN; break;
		case kChumLUp: event.type = Common::EVENT_LBUTTONUP; break;
		case kChumRDown: event.type = Common::EVENT_RBUTTONDOWN; break;
		case kChumRUp: event.type = Common::EVENT_RBUTTONUP; break;
		case kChumKeyDown:
		case kChumKeyUp:
			event.type = e.kind == kChumKeyDown ? Common::EVENT_KEYDOWN : Common::EVENT_KEYUP;
			event.kbd.keycode = (Common::KeyCode)e.key;
			event.kbd.ascii = e.ascii;
			event.kbd.flags = 0;
			return true;
		case kChumMenu: event.type = Common::EVENT_MAINMENU; return true;
		case kChumQuit: event.type = Common::EVENT_QUIT; return true;
		default: return false;
		}
		graphics->mouseEvent(e.x, e.y);
		return true;
	}

	Common::MutexInternal *createMutex() override { return new PthreadMutex(); }
	uint32 getMillis(bool) override {
		timeval now;
		gettimeofday(&now, nullptr);
		return (now.tv_sec - _start.tv_sec) * 1000 + (now.tv_usec - _start.tv_usec) / 1000;
	}
	void delayMillis(uint msecs) override { usleep(msecs * 1000); }
	void getTimeAndDate(TimeDate &td, bool) const override {
		time_t now = time(nullptr);
		struct tm t = *localtime(&now);
		td.tm_sec = t.tm_sec;
		td.tm_min = t.tm_min;
		td.tm_hour = t.tm_hour;
		td.tm_mday = t.tm_mday;
		td.tm_mon = t.tm_mon;
		td.tm_year = t.tm_year;
		td.tm_wday = t.tm_wday;
	}
	void quit() override {
		chum_shutdown();
		_exit(0);
	}
	void logMessage(LogMessageType::Type, const char *message) override {
		fputs(message, stderr);
		fflush(stderr);
	}
	void addSysArchivesToSearchSet(Common::SearchSet &, int) override {}

private:
	static void *timers(void *arg) {
		OSystem_Chumby *self = (OSystem_Chumby *)arg;
		for (;;) {
			usleep(10000);
			((DefaultTimerManager *)self->getTimerManager())->checkTimers(10);
		}
		return nullptr;
	}

	timeval _start;
	bool _sawEngine;
};

extern "C" int chum_scummvm_main(int argc, const char *const *argv) {
	g_system = new OSystem_Chumby();
	int result = scummvm_main(argc, argv);
	g_system->quit();
	return result;
}
