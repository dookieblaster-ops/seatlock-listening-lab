package com.seatlock.listeninglab

import android.app.Activity
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import android.os.Bundle
import android.widget.TextView

class MainActivity : Activity() {
    @Volatile
    private var running = false
    private var track: AudioTrack? = null
    private var writer: Thread? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val text = TextView(this)
        text.textSize = 18f
        text.setTextColor(0xFFF3EEE6.toInt())
        text.setBackgroundColor(0xFF12110E.toInt())
        text.setPadding(48, 48, 48, 48)
        setContentView(text)

        try {
            System.loadLibrary("seatlock_engine")
        } catch (error: UnsatisfiedLinkError) {
            text.text = "The Rust library did not load.\n${error.message}"
            return
        }

        val rate = 48_000
        val frames = 256
        val minBytes = AudioTrack.getMinBufferSize(
            rate,
            AudioFormat.CHANNEL_OUT_MONO,
            AudioFormat.ENCODING_PCM_FLOAT,
        )
        val audio = AudioTrack.Builder()
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_MEDIA)
                    .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                    .build(),
            )
            .setAudioFormat(
                AudioFormat.Builder()
                    .setEncoding(AudioFormat.ENCODING_PCM_FLOAT)
                    .setSampleRate(rate)
                    .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                    .build(),
            )
            .setBufferSizeInBytes(maxOf(minBytes, frames * Float.SIZE_BYTES))
            .setTransferMode(AudioTrack.MODE_STREAM)
            .build()
        track = audio
        running = true
        audio.play()
        writer = Thread {
            val buffer = FloatArray(frames)
            while (running) {
                val written = Engine.fillSilence(buffer)
                if (written > 0) {
                    audio.write(buffer, 0, written, AudioTrack.WRITE_BLOCKING)
                }
            }
        }.also { it.name = "seatlock-audio"; it.start() }

        val refresh = object : Runnable {
            override fun run() {
                if (!running) return
                text.text = "Seatlock engine\nSilence from Rust.\nFrames filled: ${Engine.framesFilled()}"
                text.postDelayed(this, 200)
            }
        }
        text.post(refresh)
    }

    override fun onDestroy() {
        running = false
        writer?.join(500)
        track?.pause()
        track?.flush()
        track?.release()
        super.onDestroy()
    }
}
