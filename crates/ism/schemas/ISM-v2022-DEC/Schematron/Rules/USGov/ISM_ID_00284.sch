<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00284">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00284][Error] All @ism:FGIsourceProtected attributes must be of type NmTokens. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain an @ism:FGIsourceProtected attribute, this rule ensures that 
	  	the FGIsourceProtected value matches the pattern defined for type NmTokens. 
	</sch:p>
	  <sch:rule id="ISM-ID-00284-R1" context="*[@ism:FGIsourceProtected]">
		    <sch:assert test="util:meetsType(@ism:FGIsourceProtected, $NmTokensPattern)" flag="error" role="error">
		    	[ISM-ID-00284][Error] All @ism:FGIsourceProtected attributes must be of type NmTokens. 
		</sch:assert>
	  </sch:rule>
</sch:pattern>