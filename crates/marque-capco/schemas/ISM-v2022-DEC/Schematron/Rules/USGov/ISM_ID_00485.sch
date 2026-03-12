<?xml version="1.0" encoding="UTF-8"?><?ICEA pattern?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00485">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:cuiSpecified="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00485][Error] All @ism:cuiSpecified attributes must be of type NmTokens. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:cuiSpecified="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain an @ism:cuiSpecified attribute, this rule ensures that the cuiSpecified value matches the pattern
		defined for type NmTokens.  
	</sch:p>
	  <sch:rule id="ISM-ID-00485-R1" context="*[@ism:cuiSpecified]">
		    <sch:assert test="util:meetsType(@ism:cuiSpecified, $NmTokensPattern)" flag="error" role="error">
		    	[ISM-ID-00485][Error] All @ism:cuiSpecified attributes must be of type NmTokens. 
		</sch:assert>
	  </sch:rule>
</sch:pattern>