<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION TYPECHECK?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00282">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00282][Error] All @ism:excludeFromRollup attributes must be of type Boolean. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain an @ism:excludeFromRollup attribute, this rule ensures 
	  	that the excludeFromRollup value matches the pattern defined for type Boolean. 
	</sch:p>
	  <sch:rule id="ISM-ID-00282-R1" context="*[@ism:excludeFromRollup]">
		    <sch:assert test="util:meetsType(@ism:excludeFromRollup, $BooleanPattern)" flag="error" role="error">
		    	[ISM-ID-00282][Error] All @ism:excludeFromRollup attributes must be of type Boolean.
		</sch:assert>
	  </sch:rule>
</sch:pattern>