package com.seatlock.listeninglab

class Engine {
    companion object {
        @JvmStatic
        external fun fillSilence(out: FloatArray): Int

        @JvmStatic
        external fun framesFilled(): Long
    }
}
