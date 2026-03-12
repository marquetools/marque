<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00340">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00340][Error] All @ism:compliesWith attributes must be of type NmTokens. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain a @ism:compliesWith attribute, this rule ensures that the @ism:compliesWith value 
	  	matches the pattern defined for type NmTokens. 
	</sch:p>
	  <sch:rule id="ISM-ID-00340-R1" context="*[@ism:compliesWith]">
		    <sch:assert test="util:meetsType(@ism:compliesWith, $NmTokensPattern)" flag="error" role="error">
		    	[ISM-ID-00340][Error] All @ism:compliesWith attributes must be of type NmTokens. 
		</sch:assert>
	  </sch:rule>
</sch:pattern>