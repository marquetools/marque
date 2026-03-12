<?xml version="1.0" encoding="utf-8"?>
<!-- **************************************************************** -->
<!--                        UNCLASSIFIED                                                        -->
<!-- **************************************************************** -->

<!-- ****************************************************************
  INTELLIGENCE COMMUNITY TECHNICAL SPECIFICATION  
  XML DATA ENCODING SPECIFICATION FOR 
  INFORMATION SECURITY MARKING METADATA (ISM.XML)
  ****************************************************************
  Module:   IC-ISM-ISOO-Rendering.xsl
  Creators: Office of the Director of National Intelligence
  Intelligence Community Chief Information Officer
  **************************************************************** -->


<!-- **************************************************************** -->
<!--                            DESCRIPTION                           -->
<!--                                                                  -->
<!-- This stylesheet renders a security banner, portion mark and      -->
<!-- control/decontrol block from a document's top-level ISM attribute-->
<!-- values. This stylesheet is to be used if a document has CUI      -->
<!-- markings; the document can be either pure CUI or commingled.     -->
<!-- This stylesheet renders the metadata in a way that is compliant  -->
<!-- with the Information Security Oversight Office (ISOO)            -->
<!-- "Marking CUI" Handbook, V1.1, Dec. 6 2016.                         -->
<!-- **************************************************************** -->

<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" version="2.0"
  xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:cve="urn:us:gov:ic:cve"
  xmlns:ism="urn:us:gov:ic:ism" xmlns:ism-func="urn:us:gov:ic:ism:functions">

  <xsl:import href="IC-ISM-SecurityBanner.xsl"/>
  <xsl:import href="IC-ISM-PortionMark.xsl"/>
  <xsl:import href="IC-ISM-ClassDeclass.xsl"/>

  <xsl:output method="text" encoding="UTF-8" media-type="text-plain" indent="no"/>
  <!-- If including this xsl causes "Content is not allowed in prolog" the importing 
  XSL is likely missing an output declaration -->

  <!-- Define variable CUIRenderingRuleSet that instructs the IC-ISM stylesheets  -->
  <!-- on what rules to use for a banner, block or portion-mark that contains     -->
  <!-- CUI markings.  In this stylesheet, CUIRenderingRuleSet is set to 'ISOO',   -->
  <!-- meaning that the ISOO rules in the CUI Marking Handbook should be followed.-->

  <xsl:param name="CUIRenderingRuleSet" select="'ISOO'" />




</xsl:stylesheet>
<!-- **************************************************************** -->
<!-- **************************************************************** -->
<!--                        UNCLASSIFIED                                                        -->
<!-- **************************************************************** -->
