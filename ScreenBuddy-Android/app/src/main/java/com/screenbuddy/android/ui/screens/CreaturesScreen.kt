package com.screenbuddy.android.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

data class CreatureInfo(
    val id: String,
    val name: String,
    val description: String,
    val personality: String,
    val color: Color,
    val emoji: String,
    val isSelected: Boolean = false
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CreaturesScreen() {
    val creatures = remember {
        listOf(
            CreatureInfo("companion-bird-01", "Companion Bird", "A friendly bird companion", "Curious", Color(0xFF4CAF50), "🐦"),
            CreatureInfo("robo-cat-01", "Robo-Cat", "A robotic cat", "Energetic", Color(0xFF2196F3), "🤖"),
            CreatureInfo("slime-king-01", "Slime King", "King of slimes", "Lazy", Color(0xFF9C27B0), "👑"),
            CreatureInfo("pixel-wizard-01", "Pixel Wizard", "A wizard made of pixels", "Neutral", Color(0xFFFF9800), "🧙"),
            CreatureInfo("cosmic-jellyfish-01", "Cosmic Jellyfish", "A jellyfish from space", "Shy", Color(0xFF00BCD4), "🪼"),
            CreatureInfo("dragon-01", "Dragon", "A mighty dragon", "Neutral", Color(0xFFF44336), "🐉"),
            CreatureInfo("ghost-01", "Ghost", "A friendly ghost", "Shy", Color(0xFF607D8B), "👻")
        )
    }

    var selectedCreature by remember { mutableStateOf(creatures.first()) }

    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Text("Creatures", style = MaterialTheme.typography.headlineMedium, modifier = Modifier.padding(bottom = 16.dp))

        // Selected creature display
        Card(modifier = Modifier.fillMaxWidth().padding(bottom = 16.dp), shape = RoundedCornerShape(16.dp)) {
            Column(modifier = Modifier.padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Box(
                    modifier = Modifier.size(80.dp).clip(CircleShape).background(selectedCreature.color.copy(alpha = 0.2f)),
                    contentAlignment = Alignment.Center
                ) {
                    Text(selectedCreature.emoji, style = MaterialTheme.typography.headlineLarge)
                }
                Spacer(modifier = Modifier.height(12.dp))
                Text(selectedCreature.name, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
                Text(selectedCreature.description, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text("Personality: ${selectedCreature.personality}", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
            }
        }

        Text("Select a creature:", style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(bottom = 8.dp))

        // Creature grid
        LazyVerticalGrid(columns = GridCells.Fixed(2), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(creatures) { creature ->
                CreatureCard(
                    creature = creature,
                    isSelected = selectedCreature.id == creature.id,
                    onClick = { selectedCreature = creature }
                )
            }
        }
    }
}

@Composable
fun CreatureCard(creature: CreatureInfo, isSelected: Boolean, onClick: () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth().clickable { onClick() }.then(if (isSelected) Modifier.border(2.dp, MaterialTheme.colorScheme.primary, RoundedCornerShape(12.dp)) else Modifier),
        shape = RoundedCornerShape(12.dp)
    ) {
        Column(modifier = Modifier.padding(16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Box(
                modifier = Modifier.size(48.dp).clip(CircleShape).background(creature.color.copy(alpha = 0.2f)),
                contentAlignment = Alignment.Center
            ) {
                Text(creature.emoji, style = MaterialTheme.typography.headlineSmall)
            }
            Spacer(modifier = Modifier.height(8.dp))
            Text(creature.name, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium)
        }
    }
}
