<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00531">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
	  	[ISM-ID-00531][Error] All resource elements with SAR markings that contain @ism:compliesWith="USGov USDOD USIC" MUST contain 
	  	only one token in @ism:SARIdentifier.  
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	If there are multiple SARs and if ism:compliesWith contains both tokens  [USIC] and [USDOD], then ERROR.
	</sch:p>
	<sch:rule id="ISM-ID-00531-R1" context="*[@ism:resourceElement='true' and @ism:SARIdentifier and $ISM_USDOD_RESOURCE and $ISM_USIC_RESOURCE]">
		<sch:assert test="util:countSARmarkings(./@ism:SARIdentifier) = 1"			
			flag="error" 
			role="error">
			[ISM-ID-00531][Error] All resource elements with SAR markings that contain @ism:compliesWith="USGov USDOD USIC attribute MUST contain 
			only one token in @ism:SARIdentifier. This allows @ism:SARIdentifier to have multiple tokens, but disallows having multiple tokens 
			and @ism:compliesWith containing both USDOD and USIC. This rule satisfies requirements specified in the IC and DoD authoritative sources  
			for SAP policies; DoD Directive 5205.07 - Special Access Program (SAP) Policy and (2) IC Markings System Register and Manual.
		</sch:assert>
	  </sch:rule>
</sch:pattern>