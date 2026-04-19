<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00382">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00382][Error] For all elements with single-valued @ism:ownerProducer, @ism:joint must NOT be true.
	</sch:p>
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		For all elements whose count of @ism:ownerProducer token values is equal to 1, @ism:joint must NOT be set to true.
	</sch:p>
	<sch:rule context="*[count(tokenize(normalize-space(string(@ism:ownerProducer)), ' ')) = 1]">
		<sch:assert test="not(@ism:joint=true())" flag="error" role="error">
			[ISM-ID-00382][Error] For all elements with single-valued @ism:ownerProducer, @ism:joint must NOT be true.
		</sch:assert>
	</sch:rule>
</sch:pattern>