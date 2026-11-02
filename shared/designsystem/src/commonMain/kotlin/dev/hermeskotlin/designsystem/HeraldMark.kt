package dev.hermeskotlin.designsystem

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

/** Mobile Goblin's emerald brand color. The original identifier keeps existing callers compatible. */
val HeraldBrandBlue = Color(0xFF33B86B)

/** A compact goblin mark for chat avatars and small in-app surfaces. */
val HeraldMark: ImageVector by lazy {
    ImageVector.Builder(name = "MobileGoblinMark", defaultWidth = 24.dp, defaultHeight = 24.dp,
        viewportWidth = 72f, viewportHeight = 72f)
        .apply {
            addPath(addPathNodes("M0,0h72v72H0z"), fill = SolidColor(Color(0xFF10251D)))
            addPath(addPathNodes("M17,31L3,23L12,48L22,47Z"), fill = SolidColor(Color(0xFF258E53)))
            addPath(addPathNodes("M55,31L69,23L60,48L50,47Z"), fill = SolidColor(Color(0xFF258E53)))
            addPath(addPathNodes("M12,34C12,17 24,10 36,10C48,10 60,17 60,34L57,51C54,61 46,67 36,67C26,67 18,61 15,51Z"),
                fill = SolidColor(Color(0xFF33B86B)))
            addPath(addPathNodes("M19,39Q27,34 33,40Q27,48 19,42Z"), fill = SolidColor(Color(0xFFFFC359)))
            addPath(addPathNodes("M53,39Q45,34 39,40Q45,48 53,42Z"), fill = SolidColor(Color(0xFFFFC359)))
            addPath(addPathNodes("M31,53Q36,57 41,53Q38,61 36,61Q34,61 31,53Z"), fill = SolidColor(Color(0xFF143329)))
        }
        .build()
}
