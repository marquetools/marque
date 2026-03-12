<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00383">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00383][Error] For elements with @ism:joint set to true, one of the values of @ism:ownerProducer must be USA.
	</sch:p>
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		For elements with @ism:joint set to true, one of the values of @ism:ownerProducer must be USA.
	</sch:p>
	<sch:rule context="*[@ism:joint=true()]">
		<sch:assert test="util:containsAnyOfTheTokens(@ism:ownerProducer, ('USA'))" flag="error" role="error">
			[ISM-ID-00383][Error] For elements with @ism:joint set to true, one of the values of @ism:ownerProducer must be USA.
		</sch:assert>
	</sch:rule>
</sch:pattern>